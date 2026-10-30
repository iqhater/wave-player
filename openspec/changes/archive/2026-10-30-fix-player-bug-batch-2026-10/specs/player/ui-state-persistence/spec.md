# Spec Delta

## Purpose

Описывает состояние интерфейса Wave Player, которое должно сохраняться между экранами и перезапусками: отображение hover-курсора, индикатор избранного текущего трека и свёрнутый sidebar.

## ADDED Requirements

### Requirement: Hover cursor visibility setting

Settings SHALL contain a checkbox at the top of the Timeline section that controls the translucent hover preview cursor on the waveform. The value MUST be stored in `config.json` as `ui.show_hover_cursor` with default `true`, MUST apply immediately, and MUST survive restart. The solid playback cursor MUST remain visible regardless of this setting.

**Verification:** unit test for serde defaults; manual smoke for the checkbox, hover preview, playback cursor, and restart.

#### Scenario: Disabling the hover cursor

- **WHEN** the user clears the "Show hover cursor" checkbox in Settings
- **THEN** moving the pointer over the waveform no longer shows the translucent preview cursor or its time label
- **AND** the solid playback cursor remains visible

#### Scenario: Enabling the hover cursor

- **WHEN** the user enables the checkbox and moves the pointer over the waveform
- **THEN** the translucent preview cursor follows the pointer and shows the preview time

#### Scenario: Setting survives restart

- **WHEN** the user disables the hover cursor and restarts the application
- **THEN** the checkbox is still cleared and the hover cursor is not shown

### Requirement: Favorite state reflects the current track on every screen

The favorite button SHALL show the favorite state of the currently playing track after any route change or remount. It MUST restore the track from persistent playback state, request the favorites snapshot on mount, and update when favorites change elsewhere. With no current track it MUST remain inactive.

**Verification:** unit test for resolution and event transitions; manual smoke for App Info and cross-screen changes.

#### Scenario: Returning from App Info

- **WHEN** the current track is a favorite and the user opens App Info and then returns to the player
- **THEN** the favorite button is still filled and removing the favorite still works

#### Scenario: Favorite changed on another screen

- **WHEN** the current track is removed from favorites on the Favorites screen
- **THEN** the favorite button in the player controls becomes unfilled, and the reverse applies after adding it back

#### Scenario: Track change resolves the new state

- **WHEN** a different track starts playing
- **THEN** the button shows that track's favorite state and never the previous track's state

#### Scenario: No current track

- **WHEN** no track has been loaded
- **THEN** the favorite button is inactive and a click does not change any stored data

### Requirement: Sidebar collapse state persistence

On desktop, the collapsed state of the sidebar SHALL be stored in `config.json` as `ui.sidebar_collapsed` with default `false`. The value MUST be saved on every toggle and restored on the next launch. The mobile overlay MUST always render expanded and MUST NOT change the persisted desktop value.

**Verification:** unit test for config roundtrip and defaults; manual smoke for collapse/expand, restart, and the mobile overlay.

#### Scenario: Collapsed state survives restart

- **WHEN** the user collapses the sidebar, closes the application, and opens it again
- **THEN** the sidebar starts collapsed

#### Scenario: Expanded state survives restart

- **WHEN** the user expands a collapsed sidebar and restarts the application
- **THEN** the sidebar starts expanded

#### Scenario: Mobile overlay does not overwrite desktop state

- **WHEN** the sidebar is expanded on desktop and the user opens and closes the mobile overlay
- **THEN** `ui.sidebar_collapsed` stays `false`

#### Scenario: Mobile overlay ignores collapsed desktop state

- **WHEN** `sidebar_collapsed` is `true` and the user opens the mobile menu
- **THEN** the overlay shows full labels and closing it leaves `sidebar_collapsed` unchanged

### Requirement: Legacy config loads with UI defaults

A `config.json` written before this change MUST load successfully. A missing `ui.show_hover_cursor` MUST default to `true`, and a missing `ui.sidebar_collapsed` MUST default to `false`. Existing stored settings MUST retain their values.

**Verification:** integration test for `ConfigStore` legacy JSON in a temporary directory.

#### Scenario: Old config without the new fields

- **WHEN** the application loads a config that has no `show_hover_cursor` and no `sidebar_collapsed` fields
- **THEN** the app starts without an error, the hover cursor is enabled, and the sidebar is expanded
- **AND** the previously stored theme, window, audio, and UI settings are unchanged
