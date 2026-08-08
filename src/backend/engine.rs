// Импортируем наши типы каналов и команд
use super::bpm_key::analyze_bpm_key;
use super::filters::LiveFilteredSource;
use super::spatial::LiveSpatialSource;

use crate::backend::generate_waveform::stream_waveform;
use crate::backend::metadata::parse_track_metadata;

use crate::shared::command_types::PlayerCommand;
use crate::shared::event_types::{AudioError, ErrorSender, PlayerEvent, StatusSender};

use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use super::decoder::load_file_with_decoder;
use super::decoder::load_samples_mono;

use rodio::{DeviceSinkBuilder, Player, decoder::DecoderBuilder};
use rodio_wsola::WsolaSourceExt;

pub fn run_audio_loop(
    // As Consumer
    cmd_rx: flume::Receiver<PlayerCommand>,

    // As Producers
    // time_tx: TimeSender,
    playback_progress: Arc<AtomicU32>,
    status_tx: StatusSender,
    error_tx: ErrorSender,
) -> Result<(), Box<dyn Error>> {
    // 1. Запускаем изолированный аудио-поток
    std::thread::spawn(move || {
        // 2. Инициализируем аудио-выход системы
        // _stream нужно хранить в памяти, иначе звук закроется сразу после инициализации
        let sink_handle = match DeviceSinkBuilder::open_default_sink() {
            Ok(res) => res,
            Err(_) => {
                if let Err(_) = error_tx.try_broadcast(AudioError::DeviceInitializationFailed) {
                    // лог канал закрыт например
                };
                return;
            }
        };

        // 3. Создаем Sink — главный пульт управления воспроизведением в rodio
        let player = Player::connect_new(&sink_handle.mixer());

        player.set_volume(0.3);

        // Source::amplify_normalized(0.4);

        // Храним метаданные текущего трека
        let mut current_track_duration = Duration::from_secs(0);
        let mut current_path: Option<PathBuf> = None;
        let mut track_start_time = Instant::now();
        let mut accumulated_time = Duration::from_secs(0); // Хранит время до пауз
        let mut has_track = false;

        let mut is_track_repeated = false;
        let mut current_speed = 1.0;
        let mut current_preserve_pitch = false;

        let mut last_time_update = Instant::now();

        // filters state
        // Live-фильтры: 0 = выключено, иначе частота среза в Hz
        let lp_cutoff = Arc::new(AtomicU32::new(0));
        let hp_cutoff = Arc::new(AtomicU32::new(0));

        // Live-пространственная обработка
        let ms_mode = Arc::new(AtomicU32::new(0)); // include mono
        let stereo_width = Arc::new(AtomicU32::new(f32::to_bits(1.0)));
        let pan = Arc::new(AtomicU32::new(f32::to_bits(0.0)));

        // loop state
        let mut is_loop = false;
        let mut current_loop_start = Duration::from_secs(0);
        let mut current_loop_end = Duration::from_secs(0);

        // Вспомогательная функция для перезапуска трека с новыми параметрами эффектов.
        // Вы можете вынести её как метод вашей структуры, передавая туда нужные переменные.
        let reload_track_with_effects =
            |
            path: &std::path::PathBuf,
            speed: f32,
            preserve_pitch: bool,
            acc_time: &mut std::time::Duration,      // передаем по ссылке
            start_time: &mut std::time::Instant,    // передаем по ссылке
        | {
            // Считаем точное текущее время воспроизведения перед остановкой
            let elapsed = start_time.elapsed();

            // Рассчитываем пройденный отрезок и обновляем оригинальную переменную через разыменование (*)
            *acc_time += elapsed.mul_f32(speed);

            // Останавливаем плеер
            player.stop();

            let file = File::open(path)
                .map_err(|_| AudioError::FileNotFound(path.to_string_lossy().into_owned()))
                .unwrap();

            let len = fs::metadata(path)
                .map_err(|_| AudioError::FileCorrupted)
                .unwrap()
                .len();

            let decoder = DecoderBuilder::new()
                .with_data(BufReader::new(file))
                .with_byte_len(len)
                .with_seekable(true)
                .with_coarse_seek(true)
                .with_hint("mp3")
                .build()
                .map_err(|_| AudioError::FileCorrupted)
                .unwrap();

            // Используем обновленное время для перемотки
            /* match decoder.try_seek(*acc_time) {
                Ok(()) => {
                    *start_time = Instant::now();
                    playback_progress.store(
                        acc_time.as_millis() as u32,
                        Ordering::Relaxed,
                    );
                }
                Err(_) => {
                    let _ = error_tx.try_broadcast(AudioError::SeekFailed);
                }
            } */

            if preserve_pitch {
                let wsola_source = decoder.wsola(speed);
                let filtered = LiveFilteredSource::new(wsola_source, lp_cutoff.clone(), hp_cutoff.clone());
                let spatial = LiveSpatialSource::new(
                    filtered,
                    ms_mode.clone(),
                    stereo_width.clone(),
                    pan.clone(),
                );
                player.append(spatial);
                // player.set_speed(speed);
            } else {
                let filtered = LiveFilteredSource::new(decoder, lp_cutoff.clone(), hp_cutoff.clone());
                let spatial = LiveSpatialSource::new(
                    filtered,
                    ms_mode.clone(),
                    stereo_width.clone(),
                    pan.clone(),
                );
                player.append(spatial);
                player.set_speed(speed);
            }
        };

        // 4. Главный бесконечный цикл обработки
        loop {
            // 1. Проверка окончания — ДО блокирующего recv
            if has_track && player.empty() {
                // let _ = time_tx.try_broadcast(current_track_duration);
                playback_progress
                    .store(current_track_duration.as_millis() as u32, Ordering::Relaxed);

                // TODO: дубль код с командой PlayerCommand::LoadFile
                if is_track_repeated {
                    // Перемотка на начало и перезапуск
                    if let Some(ref path) = current_path {
                        if let Ok((duration, ..)) = load_file_with_decoder(
                            path,
                            &player,
                            lp_cutoff.clone(),
                            hp_cutoff.clone(),
                            ms_mode.clone(),
                            stereo_width.clone(),
                            pan.clone(),
                        ) {
                            current_track_duration = duration;
                            accumulated_time = Duration::from_secs(0);
                            track_start_time = Instant::now();
                            let _ = player.try_seek(Duration::from_secs(0));
                            player.play();
                            playback_progress.store(0, Ordering::Relaxed);
                            has_track = true;

                            let _ = status_tx.try_broadcast(PlayerEvent::Started {
                                duration: current_track_duration,
                                metadata: parse_track_metadata(path),
                            });
                            continue;
                        }
                    }
                }

                let _ = status_tx.try_broadcast(PlayerEvent::Finished);
                has_track = false;

                // Переоткрываем файл и загружаем декодер, но не запускаем play
                if let Some(ref path) = current_path {
                    if let Ok((duration, ..)) = load_file_with_decoder(
                        path,
                        &player,
                        lp_cutoff.clone(),
                        hp_cutoff.clone(),
                        ms_mode.clone(),
                        stereo_width.clone(),
                        pan.clone(),
                    ) {
                        current_track_duration = duration;
                        player.pause();
                        has_track = true;
                        accumulated_time = Duration::from_secs(0);
                        track_start_time = Instant::now();
                    }
                }

                // continue — идём на следующий цикл, ждём команд
                continue;
            }

            // 2. Проверяем, играет ли звук прямо сейчас
            let is_playing = !player.is_paused() && !player.empty();

            // Динамическое чтение канала (Блокирующее если стоим, неблокирующее если играем)
            let command_result = if is_playing {
                // cmd_rx.try_recv().map(Some).unwrap_or(None)
                cmd_rx.try_recv().ok()
            } else {
                // cmd_rx.recv().map(Some).unwrap_or(None)
                match cmd_rx.recv() {
                    Ok(cmd) => Some(cmd),
                    Err(_) => {
                        // Канал закрылся, пока мы спали на паузе!
                        // Обязательно выходим из цикла, чтобы закрыть поток.
                        break;
                    }
                }
            };

            // 3. Обработка команд от Dioxus UI
            //  match command_rx.recv_timeout(Duration::from_millis(16)) {
            if let Some(command) = command_result {
                match command {
                    // PlayerCommand::LoadFile(path) => {}
                    PlayerCommand::Play(path) => {
                        player.clear();
                        accumulated_time = Duration::from_secs(0);
                        has_track = false;
                        current_path = Some(path.clone());

                        if !path.exists() {
                            let _ = error_tx.try_broadcast(AudioError::FileNotFound(
                                path.to_string_lossy().into_owned(),
                            ));
                            continue;
                        }

                        match load_file_with_decoder(
                            &path,
                            &player,
                            lp_cutoff.clone(),
                            hp_cutoff.clone(),
                            ms_mode.clone(),
                            stereo_width.clone(),
                            pan.clone(),
                        ) {
                            Ok((duration, ..)) => {
                                current_track_duration = duration;
                                player.play();

                                track_start_time = Instant::now();
                                has_track = true;

                                // get track metadata artist and title
                                let metadata = parse_track_metadata(&path);

                                let _ = status_tx.try_broadcast(PlayerEvent::Started {
                                    duration: current_track_duration,
                                    metadata,
                                });

                                // BPM/Key analysis in background
                                /* let status_tx_clone = status_tx.clone();
                                let path_clone = path.clone();
                                let limit_secs = 90;
                                std::thread::spawn(move || {
                                    if let Ok((samples, sr, _dur)) =
                                        load_samples_mono(&path_clone, limit_secs)
                                    {
                                        if let Ok(result) = analyze_bpm_key(&samples, sr, 1) {
                                            let _ = status_tx_clone.try_broadcast(
                                                PlayerEvent::BpmKeyDetected {
                                                    bpm: result.bpm,
                                                    key_name: result.key_name,
                                                    key_numerical: result.key_numerical,
                                                },
                                            );
                                        }
                                    }
                                }); */
                            }
                            Err(err) => {
                                let _ = error_tx.try_broadcast(err);
                            }
                        }
                    }
                    PlayerCommand::Pause => {
                        if !player.is_paused() {
                            // Запоминаем, сколько трек успел проиграть до нажатия паузы
                            let elapsed = track_start_time.elapsed();
                            accumulated_time += elapsed.mul_f32(current_speed);
                            player.pause();
                            // accumulated_time += track_start_time.elapsed();
                            let _ = status_tx.try_broadcast(PlayerEvent::Paused);
                        }
                    }
                    PlayerCommand::Resume => {
                        if player.is_paused() {
                            player.play();
                            // Перезапускаем точку отсчета времени
                            track_start_time = Instant::now();
                            let _ = status_tx.try_broadcast(PlayerEvent::Resumed);
                        }
                    }
                    PlayerCommand::Seek(position) => {
                        // UI/волновая форма работают в «оригинальном» времени трека.
                        // rodio внутри умножает позицию на скорость (Speed::try_seek: pos * factor),
                        // поэтому делим на скорость, чтобы декодер перемотался в нужное место файла.
                        // В режиме preserve_pitch (WSOLA) Speed не используется (factor = 1.0),
                        // поэтому деление не нужно.
                        let playback_pos = if current_preserve_pitch {
                            position
                        } else {
                            position.div_f32(current_speed)
                        };

                        // Перемотка
                        match player.try_seek(playback_pos) {
                            Ok(()) => {
                                // Обновляем счетчики времени
                                accumulated_time = position;
                                track_start_time = Instant::now();
                                // let _ = time_tx.try_broadcast(position);
                                playback_progress
                                    .store(position.as_millis() as u32, Ordering::Relaxed);
                            }
                            Err(_) => {
                                let _ = error_tx.try_broadcast(AudioError::SeekFailed);
                            }
                        }
                    }
                    PlayerCommand::SetVolume(volume) => {
                        player.set_volume(volume.clamp(0.0, 1.0));
                    }
                    PlayerCommand::SetLoop(loop_start, loop_end) => {
                        // Сохраняем параметры петли, движок сам будет перематывать
                        current_loop_start = loop_start;
                        current_loop_end = loop_end;

                        // if position == loop_end {
                        // Перематыват seek на loop_start метку
                        // match player.try_seek(loop_start) {
                        //     Ok(()) => {
                        // Обновляем счетчики времени
                        // accumulated_time = loop_start;
                        // track_start_time = Instant::now();
                        // let _ = time_tx.send(loop_start);

                        let _ = status_tx.try_broadcast(PlayerEvent::LoopSet {
                            start: loop_start,
                            end: loop_end,
                        });
                        // }
                        /* Err(_) => {
                            let _ = error_tx.send(AudioError::SeekFailed);
                        } */
                        // }
                        // }
                    }
                    PlayerCommand::ToggleLoop(loop_enabled) => {
                        is_loop = loop_enabled;

                        let _ = status_tx.try_broadcast(PlayerEvent::LoopEnabled {
                            is_loop: loop_enabled,
                        });
                    }
                    PlayerCommand::ToggleRepeatTrack(is_repeated) => {
                        is_track_repeated = is_repeated;
                        // send event to ui???
                    }
                    PlayerCommand::SetSpeed(speed) => {
                        let speed = speed.clamp(0.25, 2.0);

                        let old_speed = current_speed;
                        current_speed = speed;

                        if has_track {
                            // ВАЖНО: фиксируем прошедшее время при СТАРОЙ скорости
                            let elapsed = track_start_time.elapsed();
                            accumulated_time += elapsed.mul_f32(old_speed);
                            track_start_time = Instant::now();

                            if let Some(ref path) = current_path {
                                if current_preserve_pitch {
                                    // Если нужен WSOLA — перезапускаем поток
                                    reload_track_with_effects(
                                        path,
                                        current_speed,
                                        current_preserve_pitch,
                                        &mut accumulated_time,
                                        &mut track_start_time,
                                    );
                                } else {
                                    // Если обычный режим — просто меняем скорость «на лету» без перезапуска
                                    player.set_speed(current_speed);
                                }
                            }
                        }
                    }
                    PlayerCommand::SetPreservePitch(enabled) => {
                        // Изменяем режим, только если он реально поменялся
                        if current_preserve_pitch != enabled {
                            current_preserve_pitch = enabled;

                            if has_track {
                                if let Some(ref path) = current_path {
                                    // При смене режима ВСЕГДА перезапускаем трек,
                                    // чтобы переключиться между чистым декодером и WSOLA
                                    reload_track_with_effects(
                                        path,
                                        current_speed,
                                        current_preserve_pitch,
                                        &mut accumulated_time,
                                        &mut track_start_time,
                                    );
                                }
                            }
                        }
                    }
                    PlayerCommand::LoadFile(path) => {
                        player.stop();
                        current_path = Some(PathBuf::from(&path));

                        match load_file_with_decoder(
                            Path::new(&path),
                            &player,
                            lp_cutoff.clone(),
                            hp_cutoff.clone(),
                            ms_mode.clone(),
                            stereo_width.clone(),
                            pan.clone(),
                        ) {
                            Ok((duration, channels, sample_rate)) => {
                                current_track_duration = duration;
                                accumulated_time = Duration::from_secs(0);
                                player.play();

                                track_start_time = Instant::now();
                                playback_progress.store(0, Ordering::Relaxed);
                                has_track = true;

                                // get track metadata artist and title
                                let metadata = parse_track_metadata(Path::new(&path));

                                let _ = status_tx.try_broadcast(PlayerEvent::Started {
                                    duration: current_track_duration,
                                    metadata,
                                });

                                // Клонируем канал отправки для безопасного перемещения в функцию
                                let status_tx_clone = status_tx.clone();
                                let total_bins = 500; // Целевое количество столбиков волноформы для UI

                                // ВЫЗЫВАЕМ НАШУ НОВУЮ ВЫНЕСЕННУЮ ФУНКЦИЮ
                                // Замыкание внутри поможет адаптировать любой синтаксис отправки (например, .try_broadcast)
                                stream_waveform(
                                    PathBuf::from(&path),
                                    channels.get(),
                                    sample_rate.get(),
                                    duration,
                                    total_bins, // Целевое количество столбиков волноформы для UI
                                    move |event| {
                                        status_tx_clone
                                            .try_broadcast(event)
                                            .map(|_| ())
                                            .map_err(|e| e.into())
                                    },
                                );

                                // BPM/Key analysis in background
                                let status_tx_clone = status_tx.clone();
                                let path_clone = path.clone();
                                let limit_secs = 90;
                                std::thread::spawn(move || {
                                    if let Ok((samples, sr, _dur)) =
                                        load_samples_mono(Path::new(&path_clone), limit_secs)
                                    {
                                        if let Ok(result) = analyze_bpm_key(&samples, sr, 1) {
                                            let _ = status_tx_clone.try_broadcast(
                                                PlayerEvent::BpmKeyDetected {
                                                    bpm: result.bpm,
                                                    key_name: result.key_name,
                                                    key_numerical: result.key_numerical,
                                                },
                                            );
                                        }
                                        // drop(samples); // явно освобождаем буфер
                                    }
                                });
                            }
                            Err(err) => {
                                let _ = error_tx.try_broadcast(err);
                            }
                        }
                    }
                    PlayerCommand::SetLowPassFilter(cutoff) => {
                        lp_cutoff.store(
                            cutoff.map(|c| c.clamp(5.0, 24000.0) as u32).unwrap_or(0),
                            Ordering::Relaxed,
                        );
                    }
                    PlayerCommand::SetHighPassFilter(cutoff) => {
                        hp_cutoff.store(
                            cutoff.map(|c| c.clamp(20.0, 20500.0) as u32).unwrap_or(0),
                            Ordering::Relaxed,
                        );
                    }
                    PlayerCommand::SetMidSideMode(mode) => {
                        ms_mode.store(mode as u32, Ordering::Relaxed);
                    }
                    PlayerCommand::SetStereoWidth(width) => {
                        stereo_width.store(f32::to_bits(width.clamp(0.0, 2.0)), Ordering::Relaxed);
                    }
                    PlayerCommand::SetPan(pan_val) => {
                        pan.store(f32::to_bits(pan_val.clamp(-1.0, 1.0)), Ordering::Relaxed);
                    }
                    // TODO: add other commands
                    _ => {}
                }
            }

            // 4. Прогресс (только когда играет)
            if has_track && !player.is_paused() && !player.empty() {
                // let current_pos = player.get_pos(); // ui cursor not speed up!
                // let current_pos = accumulated_time + elapsed.mul_f32(current_speed);
                let current_pos =
                    accumulated_time + track_start_time.elapsed().mul_f32(current_speed); // ui cursor speed works fine
                let mut shifted_by_loop = false;

                // TODO: FIX BUG: петля включена-трек корректно заканчивается и сьрасывается, а если выключена трек залипает и не сбрасывается
                // DONE! Проблема в моем неверном понимании работы flume как mpmc каналов и компоненты не получали свои данные, а был нужен broadcast!
                // В итоге, flume отстался только для команд в одну сторону, а события теперь передаются через async_broadcast библиотеку
                // Проверка петли: если позиция >= loop_end → перематываем на loop_start
                if is_loop && current_loop_end > Duration::from_secs(0) {
                    /* let within_loop_end = current_pos >= current_loop_end
                    && current_pos < current_loop_end + Duration::from_millis(100)
                    && current_loop_end > Duration::from_secs(0); */

                    let playback_loop_start = if current_preserve_pitch {
                        current_loop_start
                    } else {
                        current_loop_start.div_f32(current_speed)
                    };

                    // if within_loop_end && last_time_update.elapsed() >= Duration::from_millis(50) {
                    if current_pos >= current_loop_end {
                        match player.try_seek(playback_loop_start) {
                            Ok(()) => {
                                accumulated_time = current_loop_start;
                                track_start_time = Instant::now();
                                // let _ = time_tx.try_broadcast(current_loop_start);
                                playback_progress.store(
                                    current_loop_start.as_millis() as u32,
                                    Ordering::Relaxed,
                                );
                                last_time_update = Instant::now();

                                shifted_by_loop = true; // Флаг, чтобы обычный таймер ниже не затер это событие
                            }
                            Err(_) => {
                                let _ = error_tx.try_broadcast(AudioError::SeekFailed);
                            }
                        }
                    }
                }

                if !shifted_by_loop && last_time_update.elapsed() >= Duration::from_millis(50) {
                    // let _ = time_tx.try_broadcast(current_pos);
                    playback_progress.store(current_pos.as_millis() as u32, Ordering::Relaxed);
                    last_time_update = Instant::now();
                }
            }

            if is_playing {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    });

    Ok(())
}
