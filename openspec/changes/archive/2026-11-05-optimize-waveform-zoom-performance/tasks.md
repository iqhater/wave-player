# Tasks

## 1. Пиковая пирамида в бэкенде

- [x] 1.1 В `src/backend/analysis/service.rs` добавить `WaveformPeaks { sample_rate, total_frames, bin_frames, bins: Vec<i8> }`, `PEAK_BIN_FRAMES = 128`, `MAX_PEAK_BINS = 4_000_000` и расчёт `bin_frames = max(128, next_power_of_two(ceil(total_frames / MAX_PEAK_BINS)))`.
- [x] 1.2 В `decode_overview` при `need_waveform == true` параллельно с overview накапливать min/max по всем каналам кадра; после прохода квантовать в `i8` (`round(clamp(value, -1, 1) * 127)`) в пары `min/max`.
- [x] 1.3 Обрезать число бинов по фактически прочитанным кадрам; при выходе за `MAX_PEAK_BINS` не сохранять пики, но завершать overview и LUFS без ошибки.
- [x] 1.4 Unit-тесты: число бинов и `bin_frames` для 5-минутного и часового WAV, min/max известных всплесков, квантование, cap для очень длинного файла.
- [x] 1.5 Убедиться, что overview по-прежнему содержит 500 бинов и `WaveformChunkReady` не меняется.

**Готово, когда:** `cargo test --locked --features desktop waveform_peaks` проходит; пики строятся в том же проходе, что overview, и не добавляют отдельного декодирования.

## 2. Кэш waveform версии 3

- [x] 2.1 В `src/backend/waveform_cache.rs` повысить `CACHE_VERSION` до `3`.
- [x] 2.2 Добавить `CachedWaveform { peaks, waveform_peaks }` и `CachedWaveformPeaks { sample_rate, total_frames, bin_frames, bins }`; изменить `load`/`save`.
- [x] 2.3 Расширить формат после overview: `sample_rate u32`, `total_frames u64`, `bin_frames u32`, `bin_count u32`, пары `i8 min/max`.
- [x] 2.4 Валидировать `sample_rate > 0`, `total_frames > 0`, `bin_frames > 0`, `bin_count <= MAX_PEAK_BINS`, точную длину данных и отсутствие хвоста.
- [x] 2.5 В `CachedAnalysis` добавить `#[serde(skip)] waveform_peaks`; обновить `complete()`, `persist()`, `load_cache()`.
- [x] 2.6 Integration-тесты: round-trip overview + пиков, отказ `SourceStamp::version = 2`, обрезанный файл, `bin_frames = 0`, превышение лимита, ошибка записи в недоступный каталог.
- [x] 2.7 Проверить, что старый кэш возвращает `Ok(None)` без паники и трек переанализируется.

**Готово, когда:** `cargo test --locked --features desktop waveform_cache` проходит; кэш часа занимает около 2.5 МБ, кэш без пиков считается неполным.

## 3. Событие с пиками и повторная доставка

- [x] 3.1 В `src/shared/event_types.rs` добавить `PlayerEvent::WaveformPeaksReady { sample_rate, total_frames, bin_frames, encoded: Arc<str> }`.
- [x] 3.2 В `analysis/service.rs` кодировать упакованные `i8` в base64 один раз в analysis-потоке (crate `base64` уже в зависимостях) и складывать в событие.
- [x] 3.3 Хранить последний payload в `AnalysisService` и повторять `WaveformPeaksReady` вместе с overview в `replay_overview`.
- [x] 3.4 В `waveform_bins_canvas.rs` обрабатывать событие: отправлять `encoded` и метаданные в JS через `WAVEFORM_JS`/новый скрипт; на `Started` очищать пики в JS и запрашивать overview.
- [x] 3.5 Unit-тест кодирования/декодирования payload; integration-тест replay после повторного запроса overview.
- [x] 3.6 Проверить, что broadcast не копирует многомегабайтный буфер: в событии используется `Arc<str>`.

**Готово, когда:** после анализа событие приходит один раз; после ухода с экрана и возврата пики приходят снова без повторного анализа; `cargo test --locked --features desktop waveform_peaks_ready` проходит.

## 4. Клиентская пирамида и рендер зума

- [x] 4.1 В `waveform_bins_canvas.rs` заменить текущий `animateFrame` на рендер из пиков: декодировать base64 в `Int8Array`, хранить пары `min/max`.
- [x] 4.2 Реализовать `buildPeakLevels(level0)` — один проход, каждый следующий уровень вдвое грубее; хранить `levels` и `binFrames` уровня.
- [x] 4.3 Реализовать выбор уровня: `framesPerPixel = visibleFrames / canvasWidth`, брать самый грубый уровень с `binFrames <= framesPerPixel`; если `framesPerPixel < binFrames[0]`, включать режим локального окна. Все вычисления — относительно начала вьюпорта; абсолютные кадры не приводить к `f32`.
- [x] 4.4 Рендерить по пикселям: для каждого `x` сложить min/max 1–2 бинов и нарисовать столбик; использовать `fillRect` при плотной волне, `roundRect` только когда ширина столбика больше ~3 px.
- [x] 4.5 Реализовать композицию слоёв: базовый слой на всю ширину (до ~8× — overview отдельными столбиками с зазорами, дальше mip, иначе placeholder), затем detail только в своём диапазоне. Убрать ранний `return` после detail; при частичном покрытии остальная ширина дорисовывается базовым слоем.
- [x] 4.6 Устаревший ответ `request_id` не применять к текущему вьюпорту, но класть в LRU, если его диапазон и разрешение полезны.
- [x] 4.7 Разделить played/unplayed на два прохода с одним переключением `fillStyle`; заменить LERP-проход по всем бинам общим коэффициентом появления.
- [x] 4.8 Сохранить текущие `window.waveZoom`, `window.waveScroll`, `updateWaveViewportVariables`, `zoomAt`, `panBy`, `handleWaveWheel`; перерисовку планировать через `requestAnimationFrame`.
- [x] 4.9 Добавить `window.waveformStats` (LOD, `framesPerPixel`, число fallback/detail колонок, время кадра) для manual smoke и Node-тестов; не показывать отдельный UI-оверлей.
- [x] 4.10 Node-тесты в `tests/waveform_pyramid.test.cjs` (по образцу `tests/beat_grid.test.cjs`): извлечь `WAVEFORM_JS`, замокать `canvas`/`window`/`document` и проверить `buildPeakLevels`, выбор уровня, агрегацию min/max, число `fillRect` порядка ширины canvas, отсутствие `dioxus.send` команд загрузки на wheel/pan, **отсутствие пустых колонок при частичном detail** и корректный placeholder без данных.
- [x] 4.11 Проверить выравнивание: столбик известного маркера на разных уровнях и в detail-окне попадает в одну и ту же временную позицию; первый и последний пиксели вьюпорта не выходят за границы трека.

**Готово, когда:** `node --test tests/waveform_pyramid.test.cjs` проходит; зум и панорама на часовом треке не декодируют аудио и не отправляют запросов; быстрый Zoom Out не оставляет пустых колонок.

## 5. Ограничение синхронизации вьюпорта

- [x] 5.1 Оставить `zoom`/`view_center` сигналы для loop-оверлеев и шкалы, но обновлять их не чаще одного раза примерно за 50 мс и один раз после остановки жеста.
- [x] 5.2 Убедиться, что canvas не ждёт обновления сигналов: JS перерисовывает волну в `requestAnimationFrame` сразу после `zoomAt`/`panBy`.
- [x] 5.3 Обновить `LoopPlayer`/`BeatGrid`/шкалу так, чтобы они использовали итоговый вьюпорт и не запускали загрузку данных; `BeatGrid` продолжает читать `window.waveZoom`/`window.waveScroll`.
- [x] 5.4 Node-тест: wheel/pan меняют `waveZoom`/`waveScroll` и планируют кадр, но не вызывают команды загрузки.
- [x] 5.5 Manual smoke: во время быстрого колеса волна плавная, оверлеи догоняют с задержкой не более ~100 мс, после остановки положение совпадает.

**Готово, когда:** быстрый зум не перерисовывает Dioxus-дерево на каждый тик и не запускает запросы; оверлеи приходят к итоговому вьюпорту.

## 6. Локальные окна крайнего зума

- [x] 6.1 В `src/backend/decoder.rs` добавить `SeekAccuracy { Coarse, Accurate }` и параметр в `open_file_with_decoder`; `build_audio_pipeline` передаёт `Coarse`.
- [x] 6.2 В `analysis/service.rs` вынести `decode_detail_window`: точное окно `[start, end)`, `try_seek` на `start_frame / sample_rate`, чтение ровно `frame_count` кадров, `frames_read` для тестов.
- [x] 6.3 При ошибке точного seek использовать последовательный резервный путь от начала файла, сохраняя корректные `start_frame`/точки.
- [x] 6.4 В UI запрашивать локальное окно только при `framesPerPixel < binFrames[0]`: окно `3 × видимой области`, `points = 4096`, `request_id` для вытеснения.
- [x] 6.5 Хранить в UI LRU до четырёх окон; если видимая область покрыта окном с достаточным разрешением, не отправлять запрос. При панораме внутри окна и возврате в него декодирование не повторяется.
- [x] 6.6 Коалесцировать запросы крайнего зума паузой ~50 мс; ответы на устаревшие `request_id` не применять к текущему вьюпорту, но сохранять в LRU.
- [x] 6.7 Unit/integration-тесты: окно не с нуля, маркер, `frames_read` близко к `frame_count`, резервный путь, LRU-переиспользование.
- [x] 6.8 Node-тест: при возврате в ранее загруженное окно команда загрузки не отправляется; при выходе за окно отправляется одна команда на итоговое положение.

**Готово, когда:** `cargo test --locked --features desktop decode_detail_window` и Node-тест LRU проходят; максимальный зум на часовом треке декодирует не более нескольких секунд аудио.

## 7. Проверка

- [x] 7.1 `cargo fmt --check` — без замечаний.
- [x] 7.2 `cargo check --all-targets --features desktop` — без ошибок и предупреждений, связанных с изменением.
- [x] 7.3 `cargo test --locked --features desktop` — весь набор тестов проходит.
- [x] 7.4 `node --test tests/waveform_pyramid.test.cjs` — все JS-тесты проходят.
- [ ] 7.5 Покрытие изменённых Rust-модулей при наличии `cargo llvm-cov`: `cargo llvm-cov --features desktop --lib -- backend::analysis backend::waveform_cache backend::decoder`; ориентир — не ниже 90% для новой логики.
- [x] 7.6 Опционально, с бюджетом 10 минут: `cargo mutants` для пиковой пирамиды (`WaveformPeakBuilder`, `peak_bin_frames`, `quantize_peak`) и `decode_detail_window`. Проверено 12/53 мутантов: 6 caught, 6 unviable, missed/timeout — 0; 41 не проверен — полный прогон выходит за бюджет (~25 минут).
- [x] 7.7 Manual smoke:
  - часовой MP3/WAV: после анализа зум у конца и в начале одинаково быстрый, без многосекундных зависаний;
  - быстрое колесо и панорамирование: canvas плавный, overview не мигает, детальная волна не запрашивается на каждый тик;
  - **быстрый Zoom Out после загруженного detail**: левая и правая части вьюпорта заполнены более грубым уровнем, пустых колонок нет;
  - `window.waveformStats`: при быстром зуме LOD и `framesPerPixel` меняются, fallback/detail колонки покрывают всю ширину, время кадра не растёт с длиной трека;
  - зум туда-обратно: повторные движения не декодируют аудио;
  - максимальный зум: локальное окно появляется быстро, панорама внутри окна без задержек;
  - уход с экрана и возврат: пики восстанавливаются без повторного анализа;
  - перезапуск с кэшем v3; кэш v2 и обрезанный кэш пересоздаются без блокировки воспроизведения;
  - форматы WAV, MP3 и FLAC (если доступны) не смещают волну относительно overview.
