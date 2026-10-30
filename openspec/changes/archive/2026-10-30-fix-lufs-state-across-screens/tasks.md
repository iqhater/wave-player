# Tasks

## 1. Shared LUFS state

- [x] 1.1 Добавить `integrated_lufs: Signal<Option<f32>>` в `PlaybackUIState` (`src/gui/layout.rs`), инициализировать `None` в `RootLayout` и передать в контекст; проверить `cargo check --all-targets --features desktop`.
- [x] 1.2 Добавить чистую функцию `accepted_integrated_lufs(current: Option<f32>, event: Option<f32>) -> Option<f32>` (валидность: `Some`, `is_finite`, `-60.0..=0.0`) с unit-тестами на `None`, `NaN`, `±∞`, `-60.0`, `0.0`, `-80.0`, `5.0`, замену и сохранение предыдущего значения; проверить `cargo test --locked --features desktop lufs`.

## 2. Event handling and UI

- [x] 2.1 В общем слушателе `PlayerEvent` в `RootLayout` обрабатывать `LoudnessMeasured` через `accepted_integrated_lufs`, а `Started` и `AnalysisCleared` — сбрасывать значение в `None`; проверить `cargo check --all-targets --features desktop` и unit-тестами переходов состояния.
- [x] 2.2 Переписать компонент `LUFS` в `src/gui/components/player_controls.rs` на чтение `PlaybackUIState.integrated_lufs` без локального сигнала и собственного `use_future`; проверить `cargo check --all-targets --features desktop` и grep, что `LoudnessMeasured` в GUI обрабатывается только в `layout.rs`.
- [x] 2.3 Проверить, что оба экземпляра индикатора (десктопный и мобильный) читают одно и то же состояние и форматируют значение как `{value:.1} LUFS-I`, а `None` — как «— LUFS-I».

## 3. Verification

- [x] 3.1 Выполнить `cargo fmt --check`, `cargo check --all-targets --features desktop`, `cargo test --locked --features desktop`, `node --test tests/beat_grid.test.cjs` — все проверки проходят.
- [x] 3.2 Проверить вручную: Home → App Info → Home при играющем треке сохраняет `LUFS-I`; повторные переключения экранов не сбрасывают значение; смена трека и `AnalysisCleared` показывают «— LUFS-I» до нового измерения; трек без измерения не показывает значение предыдущего.
- [x] 3.3 Проверить отсутствие изменений хранилищ: в diff нет правок схемы SQLite, `config.json` и формата кэша анализа; перезапуск приложения не восстанавливает LUFS.
- [x] 3.4 Если доступен `cargo llvm-cov`, измерить покрытие `player_controls.rs` и чистого хелпера в `layout.rs`; зафиксировать результат в summary, не гейтить общий процент.
- [x] 3.5 Проверить на узкой ширине окна, что мобильный индикатор показывает то же значение после возврата с App Info.

## Workflow follow-up

- Заархивировать чендж после review и успешной проверки.
- В summary архива указать выполненные команды, результаты coverage (или что не запускалось), manual smoke и отсутствие миграций.
