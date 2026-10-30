# Design

## Context

См. `proposal.md` — Why. Ниже только текущее состояние и ограничения, которые влияют на подход.

**Архитектура.** UI общается с аудио-движком через `PlayerCommand`/`PlayerEvent`, со store-потоком — через `StoreCommand`/`StoreEvent`. Store-поток владеет SQLite, `config.json` и кэшами (`src/backend/store/`). `RootLayout` держит разделяемые сигналы `PlaybackUIState`/`LoopUIState` (`src/gui/layout.rs`), поэтому они переживают смену экранов; локальные сигналы компонентов — нет.

**BPM.** `BpmKey` (`src/gui/components/bpm_key.rs`) сохраняет ручной BPM в `ui.manual_grids` через `PlayerContext::update_config`. `StoreCommand::UpdateTrackAnalysis` уже умеет писать `tracks.bpm`/`tracks.key`, но UI его для ручной правки не вызывает; анализ вызывает его сам (`src/backend/analysis/service.rs`). `onblur` применяет draft, но не вызывает `persist_current_grid()`. В активном `%LOCALAPPDATA%\Wave Player\config.json` есть `manual_grids` с `tempo: null` и `anchor: 32.316`, а в `tracks.bpm` лежит детектированный BPM — это подтверждает, что ручная правка не дошла до сохранения.

**Waveform/loop.** `WaveformChunkReady` приходит не потоком: `decode_overview()` декодирует трек целиком и отправляет один `overview_event`, плюс возможен один replay из кэша. Обработчик есть и в `RootLayout`, и в `WaveformBinsCanvas`; оба пишут в `PlaybackUIState.waveform_peaks`, `Waveform` читает сигнал и передаёт в canvas — это лишняя перерисовка, но не постоянный поток. Главная нагрузка — `AnalysisService`: `Latest` запускает `thread::spawn` с обычным приоритетом, а внутри последовательно идут полное декодирование, `ebur128` и `stratum_dsp::analyze_audio`. Это CPU-heavy и может душить UI/WebView во время анализа. Drag-overlay использует mouse-события, отменяет drag на `mouseleave` и может потерять `mouseup` вне окна, оставляя full-screen overlay и `dragging` включёнными — это и есть «залипание». Визуально маркеры задаёт JS через `--wave-loop-*` из `window.waveZoom/waveScroll`, а drag считает по Rust-сигналам и `cl`/`container_width` из `onmounted` — двойная модель и устаревшая геометрия дают offset.

**Favorites.** `FavoritesButton` (`src/gui/components/favorites.rs`) хранит путь, metadata и состояние избранного в локальных сигналах и узнаёт о треке только из `PlayerEvent::Started`. На `AppInfo` блок контролов скрывается (`hide_controls` в `RootLayout`), компонент размонтируется; при возврате он не знает текущий путь и не запрашивает `GetFavorites`.

**Sidebar.** `collapsed` — локальный `use_signal(false)` в `src/gui/components/sidebar.rs`; в `AppConfig` поля нет.

**Startup.** `src/main.rs` не читает `std::env::args()` и не имеет single-instance. Файл загружается только через drag&drop, файловый диалог или очередь.

**Hover cursor.** Настройки в истории git нет (`git log -S show_hover` пуст); preview добавлен безусловно в `c87fd20`. Добавляем новую настройку, а не возвращаем удалённую.

**Icon.** `build.rs` — no-op, встраивание `assets/app.rc` закомментировано; `Dioxus.toml` использует `assets/icon.png` для bundle. Проверка собранных exe: `target/dx/.../wave_player.exe` и MSI-staging показывают иконку Dioxus, `cargo build --release` — дефолтную; `bundle.icon` не меняет exe-ресурс. `assets/app.rc` уже содержит `1 ICON "favicon.ico"`.

## Goals / Non-Goals

**Goals:**

- Исправить пункты 1–7 из `bug_report.txt` (28.10.2026), не меняя архитектуру хранения BPM и не добавляя миграций SQLite.
- Сделать состояние UI восстанавливаемым после remount/перезапуска через `AppConfig` и `RootLayout`.
- Сделать анализ фоновым по приоритету, убрать дублирующиеся обновления waveform и двойную координатную модель loop-региона.
- Добавить desktop lifecycle: argv, single-instance handoff, иконку Windows.
- Покрыть новую чистую логику unit-тестами, границы модулей — integration-тестами; e2e не добавлять.

**Non-Goals:**

- Делать `tracks.bpm` единственным источником истины или удалять `ui.manual_grids`.
- Менять семантику loop/экспорта/анализа или перерисовывать waveform UX.
- Поддерживать несколько открытых файлов очередью, Android/web lifecycle, macOS Apple Events.
- Добавлять e2e/UI-автоматизацию, coverage/mutation job'ы в CI.
- Менять схему SQLite или формат существующих полей `config.json`.

## Decisions

### D1. Новые UI-настройки — в `AppConfig` и контекстах `RootLayout`

- В `UiConfig` (`src/shared/store_types.rs`) добавить:
  - `show_hover_cursor: bool` — default `true`;
  - `sidebar_collapsed: bool` — default `false`.
  Оба поля получают `#[serde(default)]` через существующий `#[serde(default)]` на `UiConfig`; `Default` реализация задаёт значения. Старые `config.json` загружаются без ошибок.
- В `RootLayout` добавить контексты `ShowHoverCursor(pub Signal<bool>)` и `SidebarCollapsed(pub Signal<bool>)` по образцу `ShowBeatGrid`/`BeatGridSnap`. Инициализировать из `ctx.config.read().ui`, обновлять в обработчике `StoreEvent::ConfigLoaded`, провайдить дочерям.
- `Settings` добавляет `SettingsCheckbox` в начало секции Timeline; обработчик пишет в контекст и вызывает `ctx.update_config(|config| config.ui.show_hover_cursor = enabled)`.
- `Waveform` читает `ShowHoverCursor` и рендерит hover-превью только при `true`; при `false` `onmousemove` не обновляет `hover_position_secs`, чтобы не вызывать лишние re-render'ы. Основной курсор не затрагивается.
- `Sidebar` читает `SidebarCollapsed` вместо локального сигнала. В desktop-ветке toggle обновляет контекст и `config.ui.sidebar_collapsed`; в mobile-ветке (когда `mobile_menu_open()`) только закрывает меню и не пишет конфиг. Для рендера использовать `compact = collapsed() && !mobile_menu_open()`, чтобы mobile overlay всегда показывал полные labels независимо от сохранённого desktop-состояния.

**Альтернативы.** Локальный сигнал + чтение конфига при старте (отклонено: не синхронизируется с `ConfigLoaded` и легко теряется при remount). Глобальный static (отклонено: не Dioxus-стиль, сложно тестировать). Хранить sidebar только в localStorage (отклонено: проект уже использует `config.json`).

### D2. Favorites восстанавливается из persistent playback state

- `FavoritesButton` больше не хранит metadata и не слушает `PlayerEvent::Started`. Источник текущего трека — `PlaybackUIState.metadata` (живёт в `RootLayout`).
- На mount и при смене пути:
  - сбросить `current_track_id = None`, `is_favorite = false`;
  - отправить `StoreCommand::GetFavorites`.
- `StoreEvent::FavoritesLoaded` резолвит текущий путь через чистую функцию `find_favorite(&tracks, &path) -> Option<(i64, bool)>`; результат задаёт `current_track_id`/`is_favorite`.
- `StoreEvent::FavoritesChanged { track_id, is_favorite }` обновляет состояние только если `track_id` совпадает с текущим. Если пришло `is_favorite = true`, а id ещё неизвестен, запрашивается `GetFavorites` для резолва по пути. Это исправляет текущее поведение, где любой `FavoritesChanged` перезаписывает id.
- Добавление в избранное строит `TrackData` из `PlaybackUIState.metadata` (title/artist/album/genre/duration/cover), без локальных копий.
- Если `metadata` нет, кнопка неактивна и клик ничего не делает.

**Альтернативы.** Оставить `PlayerEvent::Started` и дополнительно запрашивать snapshot (отклонено: два источника истины и гонка). Поднять состояние избранного в `RootLayout` (отклонено для минимального фикса: единственный потребитель — кнопка; rehydrate достаточен). `FavoritesLoaded` без привязки к пути (отклонено: даёт ложное состояние для другого трека).

### D3. Ручной BPM пишется в БД, но `manual_grids` остаётся источником override

Пользователь отказался от перестройки хранения BPM: `ui.manual_grids` остаётся, `tracks.bpm` обновляется как библиотечное значение.

- Новый `StoreCommand::SetTrackBpm { path: PathBuf, bpm: f32 }` и метод `DbStore::update_track_bpm(&path, bpm) -> rusqlite::Result<bool>` (UPDATE по `path`, без изменения `key`).
- В `BpmKey` выделяется идемпотентный `commit_bpm(...)`: повторный вызов из `onchange` после `onblur` не должен дублировать запись.
  - применить draft;
  - если значение валидно (`20..=300` и finite), обновить `grid_tempo`, сохранить `manual_grids` через `update_config`, отправить `SetTrackBpm` с `rounded_bpm(value)`; `persist` принимает распарсенное значение, а не перечитывает сигнал после записи;
  - вызывать из `onchange`, Enter и `onblur` (если draft есть и значение изменилось). `onblur` теперь тоже сохраняет — это закрывает потерю правки при клике мимо.
- `rounded_bpm(bpm) -> Option<f32>` — чистая функция: проверка диапазона/`is_finite`, затем `bpm.round()`. `format_bpm` использует то же округление, чтобы отображение и БД совпадали. В config `manual_grids.tempo` может остаться дробным для выравнивания beat grid — это разрешено спекой.
- Right-click reset: `grid_tempo = None`, сохранить `manual_grids` без tempo; если детектированный `bpm() > 0`, отправить `SetTrackBpm` с `rounded_bpm(bpm())`, иначе не писать в БД.
- На `PlayerEvent::Started`, если `manual_grids[path].tempo` есть, отправить `SetTrackBpm` с округлённым значением — это досинхронизирует БД для override'ов, созданных до фикса.
- В `StoreCommand::UpdateTrackAnalysis` (detected analysis) добавить guard:
  - store runtime держит `AppConfig` в памяти: инициализирует из `ConfigStore` при старте и обновляет на `SetConfig`. Guard читает этот снапшот, а не диск;
  - проверить `ui.manual_grids.get(path).and_then(|g| g.tempo)`;
  - если manual есть: обновить только `key` (`DbStore::update_track_key`) и отправить `TrackAnalysisUpdated { path, bpm: manual_rounded, key }`, чтобы подписчики не увидели детектированный BPM;
  - если manual нет: обновить bpm+key как сейчас.
  In-memory снапшот важен из-за гонки: `SetConfig` и `SetTrackBpm` от UI идут одним каналом, а detected update от engine может прийти между ними; к моменту `SetTrackBpm` manual override уже виден guard'у.
- `DbStore` получает `update_track_key` и/или `update_track_analysis` с `Option<f32>`; `update_track_bpm` — отдельный метод. Существующие тесты на `update_track_analysis` адаптируются.

**Альтернативы.** Сделать `tracks.bpm` каноническим и удалить tempo из `manual_grids` (отклонено пользователем). Добавить `source: Manual|Detected` в `UpdateTrackAnalysis` (работает, но расширяет контракт и требует менять анализ; отдельный `SetTrackBpm` проще). Проверять manual в analysis service (отклонено: сервис не читает config, пришлось бы дублировать доступ к store). Читать `config.json` с диска на каждый `UpdateTrackAnalysis` (отклонено: гонка с асинхронной записью `SetConfig`).

**Данные.** Схема SQLite не меняется: `tracks.bpm` уже `REAL`. `config.json` получает только новые UI-поля; `manual_grids` не меняется. Store runtime держит `AppConfig` в памяти для guard'а; файл остаётся источником персистентности между запусками.

### D4. Loop drag: фоновая нагрузка анализа, надёжный lifecycle и один источник координат

**Root cause.**

- `WaveformChunkReady` приходит не потоком: `decode_overview()` отдаёт один overview, плюс возможен один replay из кэша. Дублирующий обработчик в `RootLayout` и `WaveformBinsCanvas` даёт лишнюю перерисовку, но не постоянный шторм.
- Главная причина «вязкости на время анализа» — `AnalysisService::Latest` запускает `thread::spawn` с обычным приоритетом; поток последовательно декодирует весь трек, считает `ebur128` и `stratum_dsp::analyze_audio`. Пока это работает, UI/WebView конкурируют за CPU.
- «Залипание» — хрупкий lifecycle: drag на mouse-событиях, `onmouseleave` отменяет drag без commit/restore, а `mouseup` вне окна может не дойти; `dragging` остаётся `Some`, full-screen overlay продолжает перехватывать клики, пока пользователь не кликнет ещё раз.
- Двойная координатная модель и `cl`/`container_width` из `onmounted` дают визуальный offset/дрейф после zoom/pan/resize. Если во время анализа меняется ширина canvas (появление скроллбара, загрузка metadata/cover), drag использует старую ширину и регион движется медленнее/быстрее курсора — это тоже воспринимается как «вязкость».
- `LoopPlayer` root (`w-full max-w-[600px]`) — сосед waveform-строки; на desktop `VolumeMeter` (`w-14 shrink-0`) сужает canvas. Если маркеры и drag используют ширину loop-root, регион может заходить на meter и расходиться с canvas. Нужно измерять canvas или переместить `LoopPlayer` внутрь relative-контейнера canvas.
- JS `animateFrame` после overview крутится ~35 кадров (LERP 0.18) и на каждом eval отменяет/перезапускает rAF; это короткая конкуренция, её закрывает пауза canvas во время drag.

**Решения:**

1. **Фоновый приоритет анализа.** Добавить `thread-priority`; в `Latest::new` внутри worker-потока вызывать `set_current_thread_priority(ThreadPriority::Min)`, логировать ошибку и продолжать. Если Min слишком замедляет анализ на практике — поднять до `Crossplatform(20–30)` (below-normal). Дополнительно в циклы `decode_overview`, `decode_mono`, `analyze_detail` добавить `thread::yield_now()` на каждом `CHECK_INTERVAL` (4096 кадров), чтобы отдавать квант UI. Это основной фикс «вязкости».
2. **Изолировать peaks.** Убрать `waveform_peaks` из `PlaybackUIState` и обработчик `WaveformChunkReady` из `RootLayout`; владелец — `WaveformBinsCanvas`; `Waveform` больше не читает peaks. Один overview обновляет только canvas. Коалесер не нужен: поток не непрерывный; если позже появится streaming, добавить тогда.
3. **Надёжный drag lifecycle.** Pointer-события (`onpointerdown/move/up/cancel`) + pointer capture на overlay (или window-level `pointerup` fallback). Не отменять drag на leave; на pointerup — commit и освобождение capture/overlay; на Escape/pointercancel — восстановить anchor и освободить. Full-screen overlay не должен оставаться при отпускании вне окна.
4. **Пауза тяжёлой перерисовки canvas во время drag.** Контекст `LoopDragging`; `LoopPlayer` ставит `true` на pointerdown и `false` на pointerup/cancel; `WaveformBinsCanvas` при `true` пропускает перерисовку peaks/detail, но продолжает лёгкое обновление `--wave-cursor-pct` (только `window.waveProgress` + `updateWaveViewportVariables()`), чтобы курсор воспроизведения не замирал. После отпускания — один полный redraw с текущим progress. Короткая анимация после overview не мешает drag.
5. **Единая координатная модель.** Вынести в Rust чистую функцию `time_to_viewport_pct(time, duration, zoom, view_center) -> f32` (та же формула, что в JS: `visible = 100/zoom`, `view_start = clamp(view_center - visible/2)`, `pct = ((time/duration*100 - view_start)/visible*100).clamp(0,100)`). Использовать её для `lp`, `rp`, позиций маркеров, меток и overlap. Удалить запись `--wave-loop-*` из `updateWaveViewportVariables()` и атрибуты `data-loop-start/end`; JS оставляет только `--wave-cursor-pct` для курсора воспроизведения.
6. **Drag-математика.** Чистые функции и state machine:
   - `move_loop_region(anchor_start, anchor_end, delta_secs, duration) -> (start, end)` — сохраняет длину, клампит к `[0, duration]`;
   - `resize_loop_start(anchor_start, delta_secs, end) -> start` — клампит `[0, end-0.5]`;
   - `resize_loop_end(anchor_end, delta_secs, start, duration) -> end` — клампит `[start+0.5, duration]`;
   - `DragSession` (`begin/move/commit/cancel`) хранит anchor и режим, чтобы commit/cancel были детерминированы.
   Drag вызывает их с `delta_secs = (dx / container_width) * (duration / zoom)`, где `dx` — смещение от pointer-down.
7. **Свежая геометрия и ширина canvas.** `LoopPlayer` находится внутри relative-контейнера `waveform-canvas`. Ширина контейнера измеряется на `onmounted` и поддерживается опросом `get_client_rect()` каждые 250 мс (сигнал обновляется только при реальном изменении). `pointerdown` стартует `DragSession` синхронно по кэшированной ширине — без ожидания JS-раундтрипа, поэтому регион не отстаёт от курсора, в том числе при анализе. Это убирает offset после resize/сворачивания sidebar и расхождение с volume meter. Во время drag hover-превью отключается, чтобы не добавлять лишние перерисовки.
8. **No seek after drag.** `LoopDragging` один недостаточен: pointerup происходит раньше click, и к моменту click флаг уже `false`. Добавить `onclick: stop_propagation` на loop-root/оверлей и маркеры (или короткоживущий `suppress_click`), чтобы клик после drag не доходил до waveform seek. `LoopDragging` использовать для паузы canvas и guard'а во время drag.

**Альтернативы.** Оставить mouse-события и просто убрать `mouseleave` (отклонено: пропущенный `mouseup` вне окна всё равно оставит overlay; pointer capture надёжнее). Коалесинг chunks (отклонено: overview приходит одним событием, коалесер — лишняя сложность). Платформенное API приоритета вместо `thread-priority` (отклонено: три платформы, крейт инкапсулирует различия). `ThreadPriority::Min` vs below-normal: Min может замедлить анализ на загруженной системе; fallback описан.

**Риски.** `ThreadPriority::Min` может заметно замедлить анализ на загруженной машине; если BPM ждать слишком долго, поднять до below-normal. Pointer capture в WebView2/WebKit может отличаться; window-level `pointerup` — fallback. Пауза canvas во время drag скрывает новые peaks до отпускания — приемлемо и ожидаемо. При drag во время анализа региона всё ещё может ощущаться микрозадержка из-за общей нагрузки; это ограничение, а не отдельный источник смещения.

### D5. Desktop lifecycle: argv, single-instance, handoff

Новый модуль `src/desktop_lifecycle.rs` под `#[cfg(feature = "desktop")]`:

- `SUPPORTED_AUDIO_EXTENSIONS` — общий список (mp3, wav, flac, ogg, m4a, aac), используется и в `OpenFile`-диалоге, и в argv/handoff-валидации.
- `first_audio_file_arg<I: IntoIterator<Item = OsString>>(args: I) -> Option<PathBuf>` — пропускает exe, игнорирует аргументы, начинающиеся с `-`, проверяет `is_file()` и расширение без учёта регистра.
- `is_supported_audio_path(&Path) -> bool` — общая валидация.
- `InstanceLock`: открыть `data_dir/instance.lock`, вызвать `fs4::FileExt::try_lock()`. `Ok` — первый экземпляр, `File` держится до конца `main`; `Err(WouldBlock)` — уже запущен; прочие ошибки — залогировать и продолжить как первый экземпляр, чтобы ошибка ФС не блокировала запуск.
- Handoff: `write_open_request(dir, path)` пишет JSON `{ "path": "..." }` во временный файл и атомарно переименовывает в `open_request.json`; `take_open_request(dir)` читает и удаляет файл. JSON использует `serde_json` (уже есть). Путь сериализуется как `String` (`to_string_lossy`), это ограничение для не-UTF8 путей.
- `spawn_open_request_watcher(dir, tx)`: std-thread, опрос каждые 100 мс; при наличии файла — `take_open_request`, валидация, `tx.send(path)`. Поток живёт до выхода процесса.

`main`:

1. Вычислить `data_dir` (как сейчас).
2. Desktop: попытаться взять `InstanceLock`.
3. Если lock не взят по `WouldBlock`: взять первый валидный файл из argv; если он есть — `write_open_request` и выход. Если forwarding не удался — продолжить обычный запуск как второй экземпляр и проиграть файл (fallback).
4. Если lock взят: создать `flume::unbounded::<PathBuf>()` (open request channel), положить в него стартовый argv-файл, запустить watcher, сохранить lock guard до конца `main`.
5. Положить `open_request_rx` в `PlayerContext` (desktop-only поле).
6. `RootLayout`: хуки `use_navigator()` и `use_window()` вызываются на уровне рендера компонента и только затем переносятся в `use_future`. Вызов хука внутри `use_future`/`use_effect` нарушает правила хуков и приводит к панике `The hook list is already borrowed` при первом рендере (окно остаётся скрытым). Сам future читает `recv_async()`, валидирует путь, вызывает `navigator.push(Route::Home)`, отправляет `PlayerCommand::LoadFile`, затем фокусирует/разворачивает окно (`set_focus`/`set_minimized(false)`/`set_visible(true)`). На non-desktop поле/ветка отсутствует.

**Альтернативы.** Крейт `single-instance` (отклонено: умеет только проверку, не передаёт аргументы; старые зависимости). `interprocess` local socket (отклонено: больше кода и зависимостей для одного сценария). Только Windows named mutex (отклонено: desktop включает macOS/Linux; file lock проще и переносим).

**Риски.** macOS LaunchServices может не передавать путь через argv, а слать Apple Event — это вне scope (см. Open Questions). Handoff с lossy-путём может не найти файл с не-UTF8 именем — валидация отклонит, приложение останется работоспособным. Опрос 100 мс даёт задержку до 0.1 с — незаметно для «Открыть с помощью». Если lock-файл на сетевом диске, `try_lock` может вернуть ошибку, отличную от `WouldBlock`; тогда приложение продолжит запуск без гарантии single-instance. Focus API может отличаться между платформами; есть WebView-fallback.

### D6. Windows icon

- `build.rs`: включить `embed_resource::compile("assets/app.rc", embed_resource::ParamsIncludeDirs(&["assets"])).manifest_optional()` под `if std::env::var_os("CARGO_CFG_WINDOWS").is_some()`. Include-dir `assets` гарантирует, что `ICON "favicon.ico"` найдётся независимо от CWD `rc.exe`/`windres`. `embed-resource` уже в `[build-dependencies]`.
- `Dioxus.toml`: `[bundle] icon = ["assets/icon.png", "assets/favicon.ico"]`. CLI 0.7.10 на Windows берёт ICO для exe-ресурса (`32512 ICON`) и MSI, а PNG остаётся для Linux/macOS-упаковки. Fallback на `icon.png` не понадобился.
- **Обнаружено при реализации:** `dx` всегда встраивает собственный icon/version-ресурс в exe. Две группы иконок дают одинаковые `RT_ICON` id 1–3 и `cvtres` падает с `CVT1100` (duplicate resource). Поэтому `build.rs` пропускает встраивание, когда сборку запускает `dx`: надёжный признак — `RUSTC_WORKSPACE_WRAPPER`, указывающий на `dx.exe`. При обычном `cargo build` ресурс встраивается как раньше. Проверено: `cargo build` → `GROUP_ICON 1`, `dx bundle` → `GROUP_ICON 32512`, иконка Wave Player в обоих exe и в MSI staging.
- Runtime window icon оставить `assets/icon.png` (та же айдентика). Если после встраивания ресурса taskbar всё ещё показывает Dioxus, добавить feature `ico` к `image` и грузить `favicon.ico` в `WindowBuilder` — это fallback, не основной путь.

**Альтернативы.** `winres` (закомментированная старая зависимость; отклонено — `embed-resource` уже есть). Только bundle icon (недостаточно: exe-ресурс остаётся дефолтным).

**Проверка.** `cargo check --all-targets --features desktop` на Windows (build.rs выполнится), затем `dx bundle --release --platform desktop --package-types msi` и визуальная проверка иконки exe/MSI. Non-Windows сборка не должна запускать ресурсный шаг.

### D7. Test plan

**Unit:**

- `desktop_lifecycle`: argv с одним/несколькими файлами, флаги, отсутствующий файл, директория, регистр расширения, путь с пробелами/Unicode; `is_supported_audio_path`; `write_open_request`/`take_open_request` (temp dir); JSON roundtrip.
- `loop_player`: `time_to_viewport_pct` при zoom 1, zoom > 1, center clamp; `move_loop_region`/`resize_loop_start`/`resize_loop_end` — границы, минимальная длина, сохранение ширины, клампы; `DragSession` begin/move/commit/cancel — commit отправляет финальные значения, cancel восстанавливает anchor.
- `waveform_bins_canvas`: проверка, что analysis results обновляют только canvas-state, а не loop/controls (code review + smoke; JS/WebView не входит в unit coverage).
- `bpm_key`: `rounded_bpm` (127.4→127, 127.6→128, границы 20/300, NaN/∞); `parse_bpm` (уже есть); commit-хелпер возвращает `None` для невалидного ввода.
- `store_types`: serde roundtrip новых полей; legacy JSON без полей → `true`/`false`.
- `store/analysis`: in-memory SQLite `update_track_bpm`, `update_track_key`, `update_track_analysis`; `should_apply_detected_bpm(config, path)` — pure guard.
- `favorites`: `find_favorite(tracks, path)`.

**Integration:**

- `ConfigStore` в temp dir: старый JSON без новых полей загружается, новые поля сохраняются, повторная загрузка возвращает значения.
- Store dispatch: manual BPM → `tracks.bpm` обновлён; detected `UpdateTrackAnalysis` при manual override → bpm не меняется, key обновляется; после сброса detected применяется.
- Single-instance lock: два `File` на temp-файле — первый `try_lock` успешен, второй нет; после drop второй успешен.
- Handoff + watcher: записать запрос, поднять watcher, получить путь через канал; невалидный путь не отправляется.

**Manual smoke (WebView/audio):** drag в течение всего анализа длинного трека (не только в начале), отпускание мыши вне окна и возврат курсора, отсутствие seek после drag, правый маркер в конце трека при видимом volume meter, zoom/pan, resize/sidebar, hover toggle, возврат с App Info, рестарт с collapsed sidebar, double-click файла при закрытом/запущенном плеере, иконка exe/MSI на Windows.

**Coverage/mutation:** 90% минимум для новой/изменённой чистой логики, 95% цель; e2e нет. Mutation не обязателен для батча; если запускать, только по `desktop_lifecycle`/`loop_player`/`bpm_key` с лимитом 5 минут. Dioxus event wiring, WebView-код и OS thread priority в покрытие не входят и проверяются вручную.

## Risks / Trade-offs

- **BPM guard race** → detected `UpdateTrackAnalysis` может прийти между `SetConfig` и `SetTrackBpm`. Mitigation: in-memory `AppConfig` снапшот в store runtime и integration test на interleaving.
- **`ThreadPriority::Min`** может заметно замедлить анализ на загруженной машине. Mitigation: если BPM/анализ ждать слишком долго, поднять до below-normal (`Crossplatform(20–30)`); приоритет настраивается в одном месте.
- **Pointer capture в WebView** может отличаться между WebView2 и WebKitGTK. Mitigation: window-level `pointerup`/`pointercancel` fallback; ручной smoke на Windows как основном релизе.
- **Перенос peaks из `PlaybackUIState`** может затронуть других потребителей. Mitigation: grep подтверждает, что сигнал читают только `Waveform` и `WaveformBinsCanvas`; после изменения проверить сборку и smoke.
- **Пропуск тяжёлой перерисовки canvas во время drag** → новые peaks появляются после отпускания; курсор при этом продолжает двигаться через лёгкое обновление CSS-переменной. Mitigation: это ожидаемо; после pointerup принудительный redraw.
- **Асинхронная геометрия** → вместо `get_client_rect()` в момент pointerdown ширина кэшируется и обновляется раз в 250 мс; сразу после resize первый pointerdown может использовать ширину с задержкой до 250 мс. Mitigation: опрос обновляет сигнал до начала следующего drag; для hover и click `Waveform` тоже пересчитывает rect опросом и якорится на client-координаты, а не на `element_coordinates()` (они относительны элемента под курсором, например региона петли).
- **Favorites rehydration** → несколько `GetFavorites` при быстрой навигации; `FavoritesLoaded` не содержит request id. Mitigation: после загрузки резолвить по текущему пути и игнорировать снапшот, если путь уже сменился.
- **Single-instance file lock** → поведение на сетевых/экзотических ФС; handoff-файл может остаться после краха. Mitigation: lock в локальном data dir, watcher удаляет handoff после чтения, при старте старый handoff игнорируется/удаляется.
- **macOS open-with** → LaunchServices может не передать argv. Mitigation: вне scope, зафиксировано в Open Questions; Windows — основной релиз.
- **ICO в `dx bundle`** → CLI может не принять `.ico`. Mitigation: fallback на PNG для установщика при корректном exe-ресурсе.
- **`rc.exe`/`windres`** → сборка Windows без ресурсного компилятора упадёт. Mitigation: CI Windows MSVC содержит `rc.exe`; локальная проверка в задаче; при необходимости — документировать требование.

## Migration Plan

- **Config:** миграции нет. Новые поля читаются с defaults; при следующем сохранении записываются в `config.json`. Откат на старую версию безопасен: старый код игнорирует неизвестные поля.
- **SQLite:** миграции нет. `tracks.bpm` уже существует; ручные значения пишутся в него. Откат не требует восстановления данных.
- **Lifecycle files:** `instance.lock` и `open_request.json` создаются в data dir. Они не являются пользовательскими данными; старые версии их игнорируют. При желании пользователь может удалить их вручную.
- **Deploy:** обычный релиз; специальных шагов нет.

## Open Questions

- Нужна ли поддержка macOS LaunchServices open-document (Apple Events), если Finder не передаёт путь через argv? Сейчас вне scope; проверяется вручную на macOS, при необходимости — отдельный чендж.
- Принимает ли `dx bundle` `.ico` в `[bundle] icon` на текущей версии 0.7.10? Проверяется задачей; fallback уже описан и не меняет поведение exe-иконки.
- Разрешается: CLI принимает ICO; дополнительно `dx` сам встраивает ресурс, поэтому `build.rs` не должен дублировать иконку при dx-сборке (см. D6).
