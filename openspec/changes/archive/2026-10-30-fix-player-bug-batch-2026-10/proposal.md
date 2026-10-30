# Proposal

## Why

`bug_report.txt` (секция `28.10.2026`, пункты 1–7) описывает набор регрессий, которые ломают доверие к сохранности данных и отзывчивости плеера: ручной BPM не попадает в БД, loop-регион «вязнет» и залипает во время анализа нового трека, нет переключателя hover-курсора (в истории git настройки не найдено), индикатор избранного исчезает после экрана App Info, `sidebar_collapsed` не переживает перезапуск, «Открыть с помощью…» не запускает файл, а Windows-релиз показывает иконку Dioxus.

Пункты 3–5 — одна тема: состояние UI должно быть общим и переживать навигацию/перезапуск. Пункты 6–7 относятся к жизненному циклу desktop-приложения и упаковке. Поэтому фиксим их одним батчем, не расширяя scope на более ранние идеи из `bug_report.txt`.

## What Changes

- **BPM (п. 1).** Ручная правка BPM сохраняется в `tracks.bpm` в округлённом виде (как отображается). Существующий manual override в `ui.manual_grids` сохраняется и остаётся приоритетным. `Reanalyze` не перезаписывает ручной BPM; сброс (right-click) возвращает детектированное значение и синхронизирует его с БД.
- **Loop drag (п. 2).** Анализ переводится на worker-потоки с минимальным OS-приоритетом, чтобы декодирование, BPM/key и loudness не конкурировали с UI. Drag переводится на pointer-события с надёжным завершением (pointerup/pointercancel/Escape), без отмены на mouseleave; убираются двойной источник координат и устаревшая геометрия. Loop-регион привязывается к ширине waveform canvas, а не к внешнему контейнеру с volume meter; результат waveform-анализа обновляет только canvas, BPM/key/loudness — свои индикаторы, loop/controls не перестраиваются.
- **Hover cursor (п. 3).** В `config.json` добавляется `ui.show_hover_cursor` (default `true`), в начале секции Timeline экрана Settings — чекбокс. Переключатель скрывает только полупрозрачный курсор-превью; основной курсор воспроизведения остаётся видимым.
- **Favorites (п. 4).** Индикатор избранного восстанавливается из `PlaybackUIState` и снапшота `GetFavorites` при монтировании, а не из локального состояния, зависящего от `PlayerEvent::Started`. Состояние сохраняется на всех экранах и обновляется при добавлении/удалении трека.
- **Sidebar (п. 5).** В `config.json` добавляется `ui.sidebar_collapsed` (default `false`). Desktop-состояние сохраняется при toggle и восстанавливается при старте; мобильный overlay всегда expanded и не меняет сохранённое desktop-состояние.
- **Open with (п. 6).** Desktop-запуск читает argv: первый существующий поддерживаемый аудиофайл загружается и запускается автоматически после готовности UI. Реализуется single-instance: второй процесс передаёт путь уже запущенному экземпляру и завершается; запущенный экземпляр открывает Home, запускает трек и фокусирует окно.
- **Windows icon (п. 7).** `build.rs` встраивает `assets/app.rc` → `assets/favicon.ico` в Windows executable. `favicon.ico` добавляется в bundle-конфиг для установщика, если `dx` поддерживает `.ico`; иначе exe/taskbar получают ico через ресурс, а установщик остаётся на текущем PNG.
- **Совместимость.** Новые поля `config.json` получают `Default`/serde defaults; старые файлы загружаются без ошибок. Схема SQLite не меняется, миграция не требуется: используется существующая колонка `tracks.bpm`.

## Capabilities

### New Capabilities

- `player/ui-state-persistence`: переключатель hover-курсора, индикатор избранного на всех экранах и сохранение `sidebar_collapsed` в `config.json`.
- `player/library-metadata`: сохранение ручного BPM в БД с приоритетом над результатом повторного анализа.
- `player/waveform-loop`: отзывчивое перетаскивание loop-региона во время анализа и единая координатная модель waveform.
- `player/app-lifecycle`: открытие аудиофайла из argv, single-instance handoff и иконка Windows-приложения.

### Modified Capabilities

Нет: `openspec/specs/` пока пуст, существующих capability-спек нет.

## Impact

**UI:** `src/gui/components/bpm_key.rs`, `waveform.rs`, `waveform_bins_canvas.rs`, `loop_player.rs`, `favorites.rs`, `sidebar.rs`; `src/gui/layout.rs`; `src/gui/screens/settings.rs`.

**Store/backend:** `src/backend/store/analysis.rs`, `src/backend/store/dispatch.rs`, `src/shared/command_types.rs`, `src/shared/store_types.rs`.

**Lifecycle/packaging:** `src/main.rs`, новый desktop-lifecycle модуль (argv, lock/handoff, watcher), `build.rs`, `Dioxus.toml`, `assets/app.rc`.

**Зависимости:** `fs4` с feature `sync` для межпроцессного file lock и `thread-priority` для фонового приоритета анализа. Новых dev-зависимостей не требуется.

**Тесты:** unit-тесты для парсинга argv, drag/viewport-математики, округления BPM, config defaults и handoff-протокола; integration-тесты на in-memory SQLite и временных каталогах. E2E не добавляются.

## Non-goals

- Пересмотр архитектуры хранения BPM (делать `tracks.bpm` единственным источником истины, удалять `ui.manual_grids`) — не сейчас.
- Ранние пункты `bug_report.txt`: плавные low/high-pass, слайдер скорости, плейлисты, Camelot, кнопка собственного BPM.
- Single-instance и argv-обработка для Android/web.
- Автоматические e2e/UI-тесты и новые CI job'ы для coverage/mutation.
- Изменение семантики loop-региона, экспорта или анализа; перерисовка waveform/UX за пределами отзывчивости drag.
- Очередь из нескольких файлов, открываемых через «Открыть с помощью…».

## Assumptions

- Подтверждено пользователем: «BPM пока что не трогаем» означает отказ от перестройки хранения BPM, а не отказ от исправления п. 1: manual override в `ui.manual_grids` остаётся, в БД пишется округлённое значение.
- «Вязкость» loop-региона воспроизводится только во время анализа нового трека; регион не прыгает. Основные причины — CPU-нагрузка фонового анализа на обычном приоритете и хрупкий lifecycle drag (mouseleave/пропущенный mouseup); визуальный offset — отдельный латентный дефект координатной модели.
- Подтверждено пользователем: hover-курсор — это полупрозрачный preview-курсор за мышью, выглядящий как курсор воспроизведения. Настройки в истории git не было; добавляем новую.
- Ручной BPM приоритетнее результата `Reanalyze`; детектированный BPM можно вернуть только явным сбросом.
- Single-instance достаточно на desktop (Windows/macOS/Linux); при неудачной передаче пути второй процесс может продолжить обычный запуск, чтобы не терять действие пользователя.
- Иконка `favicon.ico` — это та же айдентика, что `icon.png`; runtime-иконка окна может остаться на PNG.
