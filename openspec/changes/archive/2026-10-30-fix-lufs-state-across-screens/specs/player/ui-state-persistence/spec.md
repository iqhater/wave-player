# Spec Delta

## ADDED Requirements

### Requirement: Integrated loudness survives screen changes

Индикатор `LUFS-I` в панели управления SHALL показывать интегрированную громкость текущего трека после любой смены экрана или remount компонента. Значение MUST храниться в состоянии уровня layout и MUST NOT зависеть от того, что конкретный смонтированный компонент получил событие анализа.

**Verification:** unit-тест чистой логики обновления/сброса значения; manual smoke — Home → App Info → Home.

#### Scenario: Returning from App Info

- **WHEN** трек играет, LUFS уже измерен, пользователь открывает App Info и возвращается в плеер
- **THEN** индикатор показывает то же значение `LUFS-I`, что и до перехода

#### Scenario: Repeated screen changes

- **WHEN** пользователь несколько раз переключается между Home, Favorites и App Info
- **THEN** значение не пропадает и не меняется, пока трек не сменится или анализ не будет очищен

#### Scenario: Mobile volume controls

- **WHEN** на узкой ширине отображается мобильный регулятор громкости
- **THEN** его индикатор `LUFS-I` показывает то же значение, что и десктопный

### Requirement: Integrated loudness resets with the track

Значение `LUFS-I` MUST сбрасываться в неопределённое состояние при запуске нового трека и при очистке анализа, чтобы не показывать громкость предыдущего трека. Новое значение MUST появляться после события измерения для текущего трека.

**Verification:** unit-тест переходов состояния; manual smoke со сменой трека.

#### Scenario: New track starts

- **WHEN** запускается другой трек
- **THEN** индикатор немедленно показывает «— LUFS-I»
- **AND** после завершения измерения показывает громкость нового трека

#### Scenario: Analysis cleared

- **WHEN** приходит событие очистки анализа
- **THEN** индикатор показывает «— LUFS-I»

#### Scenario: Silent or unmeasured track

- **WHEN** для трека не удалось измерить интегрированную громкость
- **THEN** индикатор остаётся в состоянии «— LUFS-I» и не показывает значение предыдущего трека

### Requirement: Integrated loudness ignores invalid measurements

Индикатор MUST принимать только конечные значения в диапазоне `-60.0..=0.0`. События с `None`, `NaN`, бесконечностью или значением вне диапазона MUST NOT заменять последнее валидное значение и MUST NOT отображаться.

**Verification:** unit-тесты валидации (границы, `None`, `NaN`, бесконечность).

#### Scenario: Out-of-range value

- **WHEN** приходит измерение `-80.0` или `5.0`
- **THEN** индикатор сохраняет предыдущее валидное значение

#### Scenario: Valid boundary values

- **WHEN** приходит измерение ровно `-60.0` или `0.0`
- **THEN** индикатор показывает это значение с одним знаком после запятой

#### Scenario: Non-finite value

- **WHEN** приходит `NaN` или бесконечность
- **THEN** индикатор не меняется

### Requirement: Integrated loudness is session state

`LUFS-I` MUST быть сессионным состоянием: значение MUST NOT записываться в `config.json`, SQLite или кэш анализа и MUST NOT восстанавливаться после перезапуска приложения. Формат `config.json`, схема SQLite и формат кэша анализа MUST остаться неизменными.

**Verification:** code review отсутствия записи в хранилища; manual smoke после перезапуска.

#### Scenario: Restart

- **WHEN** пользователь перезапускает приложение и запускает тот же трек
- **THEN** до нового измерения индикатор показывает «— LUFS-I»
- **AND** `config.json`, база и кэш не содержат новых полей LUFS

#### Scenario: Existing stored data

- **WHEN** приложение обновляется на эту версию со старым `config.json` и базой
- **THEN** они загружаются без ошибок и миграций
