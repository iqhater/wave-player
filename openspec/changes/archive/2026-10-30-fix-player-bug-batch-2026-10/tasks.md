# Tasks

## 1. Persistent UI settings

- [x] 1.1 Добавить `show_hover_cursor: bool` (default `true`) и `sidebar_collapsed: bool` (default `false`) в `UiConfig`/`Default` в `src/shared/store_types.rs`; проверить `cargo test --locked --features desktop config_tests` — roundtrip и legacy defaults проходят.
- [x] 1.2 Добавить контексты `ShowHoverCursor` и `SidebarCollapsed` в `src/gui/layout.rs`, инициализировать из `ctx.config.read().ui` и синхронизировать в обработчике `StoreEvent::ConfigLoaded`; проверить `cargo check --all-targets --features desktop`.
- [x] 1.3 Добавить чекбокс "Show hover cursor" в начало Timeline-секции `src/gui/screens/settings.rs`; в `src/gui/components/waveform.rs` рендерить hover-превью только при включённой настройке, а при выключенной не обновлять `hover_position_secs` в `onmousemove`; проверить вручную: выключение убирает превью, но не основной курсор.
- [x] 1.4 Перевести `src/gui/components/sidebar.rs` на контекст `SidebarCollapsed`; сохранять `config.ui.sidebar_collapsed` только в desktop-ветке toggle, mobile-ветка только закрывает меню; для mobile overlay использовать `compact = collapsed() && !mobile_menu_open()`, чтобы overlay всегда был expanded; проверить вручную и инспекцией `config.json`.
- [x] 1.5 Добавить интеграционный тест `ConfigStore` на temp dir: старый JSON без новых полей загружается с `true`/`false`, новые поля сохраняются и читаются обратно; проверить `cargo test --locked --features desktop config_store_`.
- [x] 1.6 Проверить вручную: hover toggle переживает перезапуск; collapsed sidebar восстанавливается после закрытия/открытия; на мобильной ширине закрытие overlay не меняет `sidebar_collapsed`.

## 2. Favorites across screen changes

- [x] 2.1 Переписать `src/gui/components/favorites.rs`: брать текущий трек из `PlaybackUIState.metadata`, не хранить локальные metadata-копии и не зависеть от `PlayerEvent::Started`; на mount и смене пути отправлять `StoreCommand::GetFavorites`; проверить `cargo check --all-targets --features desktop`.
- [x] 2.2 Вынести чистую логику выбора избранного (`find_favorite`, обработка `FavoritesLoaded`/`FavoritesChanged`, необходимость refresh при неизвестном id) в тестируемые функции и покрыть unit-тестами; проверить `cargo test --locked --features desktop favorites`.
- [x] 2.3 Исправить обработку `FavoritesChanged`: обновлять состояние только при совпадении `track_id`, при добавлении без известного id запрашивать `GetFavorites`; строить `TrackData` из `PlaybackUIState.metadata`; проверить unit-тестами переходов состояния.
- [x] 2.4 Проверить вручную: избранный трек, переход Home → App Info → Home — иконка остаётся закрашенной; удаление/добавление на экране Favorites обновляет кнопку; смена трека не показывает состояние предыдущего; без трека кнопка неактивна.

## 3. Manual BPM persistence

- [x] 3.1 Добавить `rounded_bpm(bpm) -> Option<f32>` и использовать одно округление в `format_bpm` и при сохранении; покрыть unit-тестами `127.4→127`, `127.6→128`, границы `20/300`, NaN/∞; проверить `cargo test --locked --features desktop bpm_input_tests`.
- [x] 3.2 Добавить `StoreCommand::SetTrackBpm { path, bpm }` и `DbStore::update_track_bpm` в `src/backend/store/analysis.rs`; покрыть integration-тестом на in-memory SQLite (обновление существующего трека, отсутствующий трек); проверить `cargo test --locked --features desktop update_track_bpm`.
- [x] 3.3 Добавить `DbStore::update_track_key` и guard в `StoreCommand::UpdateTrackAnalysis`: store runtime держит `AppConfig` в памяти (инициализация из `ConfigStore`, обновление на `SetConfig`), при наличии `manual_grids[path].tempo` обновлять только key и отправлять manual BPM через `StoreEvent`; вынести `should_apply_detected_bpm` в чистую функцию и покрыть unit + integration тестами, включая interleaving `SetConfig` → detected update; проверить `cargo test --locked --features desktop`.
- [x] 3.4 Сделать единый идемпотентный commit-путь в `src/gui/components/bpm_key.rs` для `onchange`, Enter и `onblur`: применить draft, сохранить `manual_grids`, отправить `SetTrackBpm` с округлённым значением только при изменении; right-click reset очищает override и отправляет детектированное значение, если оно есть; проверить unit-тестами commit-хелпера и вручную.
- [x] 3.5 Проверить вручную: ввод `127.4` + Enter/клик мимо сохраняет `127` в БД и после перезапуска; существующий `manual_grids[path].tempo` досинхронизируется в БД при старте трека; `Reanalyze` не меняет ручной BPM; right-click возвращает детектированный; невалидный ввод и Escape ничего не пишут.

## 4. Loop drag and analysis responsiveness

- [x] 4.1 Убрать `waveform_peaks` из `PlaybackUIState` и обработчик `WaveformChunkReady` из `RootLayout`; сделать `WaveformBinsCanvas` владельцем peaks; проверить `cargo check --all-targets --features desktop` и grep, что других потребителей сигнала нет.
- [x] 4.2 Добавить `thread-priority` и выставлять `ThreadPriority::Min` в worker-потоках `AnalysisService::Latest` (tracks и details), логировать ошибку и продолжать; добавить `thread::yield_now()` на каждом `CHECK_INTERVAL` в циклах `decode_overview`/`decode_mono`/`analyze_detail`; проверить `cargo check --all-targets --features desktop` и вручную, что анализ завершается и BPM приходит.
- [x] 4.3 Добавить контекст `LoopDragging` в `RootLayout`; выставлять его в `LoopPlayer` на pointerdown/pointerup/cancel; в `WaveformBinsCanvas` при drag пропускать тяжёлую перерисовку peaks/detail, но продолжать лёгкое обновление `--wave-cursor-pct`, чтобы курсор воспроизведения не замирал; после отпускания делать один полный redraw; проверить вручную drag во время анализа.
- [x] 4.4 Реализовать `time_to_viewport_pct` в Rust и использовать для `lp`, `rp`, позиций маркеров и меток; удалить запись `--wave-loop-*` и `data-loop-start/end` из `waveform_bins_canvas.rs`; покрыть unit-тестами zoom 1, zoom > 1, clamp center; проверить `cargo test --locked --features desktop loop_`.
- [x] 4.5 Переместить `LoopPlayer` внутрь relative-контейнера waveform (или измерять именно `waveform-canvas`), чтобы ширина loop-root совпадала с canvas при видимом `VolumeMeter`; реализовать чистые `move_loop_region`/`resize_loop_start`/`resize_loop_end` и `DragSession` (begin/move/commit/cancel); подключить pointer-события с pointer capture и window-level `pointerup` fallback; не отменять drag на leave; снимать геометрию в pointerdown через `get_client_rect`, игнорировать движения до готовности; `SetLoop` только на pointerup, cancel восстанавливает anchor; покрыть unit-тестами границы, минимальную длину, сохранение ширины, commit/cancel; проверить `cargo test --locked --features desktop`.
- [x] 4.6 Не допускать seek после drag: `LoopDragging` использовать для паузы canvas, а на loop-root/оверлее и маркерах добавить `onclick: stop_propagation` (или короткоживущий `suppress_click`), потому что pointerup приходит раньше click; проверить вручную: клик после drag не перематывает трек.
- [x] 4.7 Проверить вручную: drag в течение всего анализа длинного трека (не только в начале), отпускание мыши вне окна и возврат курсора, отсутствие seek после drag, правый маркер в конце трека при видимом volume meter (не заходит на meter), zoom 1 и zoom/pan, resize окна и collapse sidebar между drag'ами; Escape отменяет и не шлёт `SetLoop`; hover toggle не влияет на drag.
- [x] 4.8 Замерить длительность анализа с `ThreadPriority::Min`; если анализ стал заметно медленнее (например, более чем в 2 раза), переключить на `Crossplatform(20–30)` (below-normal) и повторить smoke; результат зафиксировать в summary.

## 5. Desktop open-with and single instance

- [x] 5.1 Добавить `fs4` с feature `sync` в `Cargo.toml`, создать `src/desktop_lifecycle.rs` под `#[cfg(feature = "desktop")]` с `SUPPORTED_AUDIO_EXTENSIONS`, `first_audio_file_arg` и `is_supported_audio_path`; покрыть unit-тестами (один/несколько файлов, флаги, отсутствующий файл, директория, регистр расширения, пробелы/Unicode); проверить `cargo test --locked --features desktop desktop_lifecycle`.
- [x] 5.2 Реализовать `InstanceLock` на `fs4::FileExt::try_lock` для `data_dir/instance.lock` (Ok — первый, `WouldBlock` — уже запущен, прочие ошибки логируются и не блокируют старт), атомарную запись/чтение `open_request.json` и watcher-поток с опросом 100 мс; покрыть integration-тестами lock (второй `try_lock` fails, после drop succeeds) и handoff (write/take, невалидный путь); проверить `cargo test --locked --features desktop`.
- [x] 5.3 Встроить lifecycle в `src/main.rs`: вычислить data dir, взять lock, при занятом lock передать первый валидный argv-файл и выйти; при неудачной передаче — fallback на обычный запуск; для первого экземпляра положить стартовый argv-файл в `flume`-канал, запустить watcher и держать lock guard до конца `main`; проверить `cargo check --all-targets --features desktop` и вручную.
- [x] 5.4 Добавить `open_request_rx` в `PlayerContext` (desktop-only) и обработчик в `RootLayout`: валидировать путь, `navigator().push(Route::Home)`, отправить `PlayerCommand::LoadFile`, сфокусировать/развернуть окно; проверить вручную: файл при закрытом и запущенном плеере.
- [x] 5.5 Перевести фильтр `OpenFile` в `src/gui/components/open_file.rs` на общий `SUPPORTED_AUDIO_EXTENSIONS`; проверить, что диалог по-прежнему открывает mp3/wav/flac/ogg/m4a/aac.
- [x] 5.6 Обновить `README.md`: описать открытие файла из контекстного меню, single-instance и ограничение macOS LaunchServices/Apple Events; проверить, что команды и разделы README не противоречат поведению.
- [x] 5.7 Проверить вручную: двойной клик по файлу при закрытом плеере запускает трек; при запущенном — фокусирует существующее окно, переходит на Home и играет; второй запуск без файла не меняет воспроизведение; отсутствующий/неподдерживаемый путь игнорируется.

## 6. Windows icon

- [x] 6.1 Включить `embed_resource::compile("assets/app.rc", embed_resource::ParamsIncludeDirs(&["assets"])).manifest_optional()` в `build.rs` под `CARGO_CFG_WINDOWS`; проверить `cargo check --all-targets --features desktop` на Windows — ресурсный шаг находит `favicon.ico` и проходит без ошибок.
- [x] 6.2 Добавить `assets/favicon.ico` в `[bundle] icon` в `Dioxus.toml`; собрать `dx bundle --release --platform desktop --package-types msi` и проверить иконку exe и MSI; если `dx` не принимает `.ico`, вернуть `icon.png` для установщика и зафиксировать fallback в design/README.
- [x] 6.3 Проверить, что ресурсный шаг не запускается на не-Windows (code review cfg + CI matrix); локально на Windows дополнительно убедиться, что debug и release сборки используют `assets/favicon.ico`.
- [ ] 6.4 Проверить вручную: Explorer, taskbar и окно приложения показывают иконку Wave Player, а не Dioxus, в release-сборке.

## 7. Integration verification

- [x] 7.1 Выполнить `cargo fmt --check` и исправить форматирование только в изменённых файлах.
- [x] 7.2 Выполнить `cargo check --all-targets --features desktop` — без ошибок и предупреждений (CI использует `-D warnings`).
- [x] 7.3 Выполнить `cargo test --locked --features desktop` — все Rust-тесты проходят, включая новые unit/integration.
- [x] 7.4 Выполнить `node --test tests/beat_grid.test.cjs` — JS-тесты проходят.
- [x] 7.5 Если доступен `cargo llvm-cov`, измерить покрытие изменённых модулей (`desktop_lifecycle`, `loop_player`, `waveform_bins_canvas`, `bpm_key`, `store/analysis`, `store_types`); зафиксировать результат в summary, не гейтить глобальный процент.
- [ ] 7.6 Опционально: запустить `cargo-mutants` только по новым чистым модулям (`desktop_lifecycle`, drag/viewport-математика в `loop_player`, `bpm_key`) с лимитом 5 минут и per-mutant timeout; при превышении остановиться и записать, что не проверено. Для багфикс-батча mutation не обязателен.
- [ ] 7.7 Пройти общий manual smoke: drag в течение всего анализа, отпускание мыши вне окна, hover toggle, App Info favorites, sidebar restart, open-with закрытого/запущенного плеера, Windows icon; записать результаты (включая пропуски) в summary.
- [x] 7.8 Убедиться, что в чендже нет e2e-тестов, новых CI job'ов и изменений схемы SQLite; при обнаружении — убрать или вынести в отдельный чендж.

## Workflow follow-up

- Заархивировать чендж после review и успешной интеграционной проверки.
- В summary архива указать фактически выполненные команды, результаты coverage/mutation (или что они не запускались), manual smoke, выбранный приоритет анализа и известные ограничения (macOS Apple Events, ICO в bundle).
