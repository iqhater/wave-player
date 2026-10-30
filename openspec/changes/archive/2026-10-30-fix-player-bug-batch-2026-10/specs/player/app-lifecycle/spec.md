# Spec Delta

## Purpose

Описывает desktop-запуск Wave Player: открытие аудиофайла из аргументов командной строки, передачу пути уже запущенному экземпляру и иконку Windows-приложения.

## ADDED Requirements

### Requirement: Desktop startup opens the first audio file argument

On desktop startup, if the first non-flag command-line argument points to an existing supported audio file, the app SHALL load and play it automatically after the UI is ready. If no valid file is supplied, the app SHALL start normally without playback. Only the first valid file MUST be used; additional arguments MUST be ignored.

**Verification:** unit test for argv parsing and path validation; manual smoke for a closed app, no args, paths with spaces, and multiple files.

#### Scenario: Opening a file while the app is closed

- **WHEN** the user double-clicks an MP3 file and Wave Player is not running
- **THEN** the player window opens and the file starts playing

#### Scenario: Starting without arguments

- **WHEN** the app is launched without a file argument
- **THEN** it opens normally and does not start playback

#### Scenario: Path with spaces or non-ASCII characters

- **WHEN** the argument is an existing file whose path contains spaces or non-ASCII characters
- **THEN** the file is recognized and played

#### Scenario: Invalid or unsupported argument

- **WHEN** the argument is missing, is a directory, or has an unsupported extension
- **THEN** the app starts normally without playback and without an error dialog

#### Scenario: Multiple arguments

- **WHEN** several paths are passed
- **THEN** the first existing supported audio file is played and the others are ignored

### Requirement: Single instance forwards open requests

Only one desktop instance SHALL run at a time. If the app is already running and a new process is launched with an audio file, the new process MUST forward the path to the running instance and exit. The running instance MUST navigate to the player screen, load and play the file, and bring its window to the foreground.

**Verification:** integration test for the instance lock and handoff file; manual smoke for a running instance, a second launch without a file, and the fallback path.

#### Scenario: Opening a file while the app is running

- **WHEN** the app is running on the Settings screen and the user double-clicks an audio file
- **THEN** the existing window comes to the foreground, navigates to the player, and plays the file
- **AND** no second player window remains open

#### Scenario: Opening another file while playing

- **WHEN** a track is already playing and a new file is opened
- **THEN** the running instance replaces the current track with the new one

#### Scenario: Forwarding fails

- **WHEN** the running instance cannot receive the forwarded path
- **THEN** the new process falls back to a normal startup and plays the file, so the user action is not lost

#### Scenario: Second launch without a file

- **WHEN** the app is already running and a new process is launched without a file
- **THEN** the running instance remains usable and its playback is not changed

#### Scenario: Previous instance exited

- **WHEN** the previous instance has exited and a new launch occurs
- **THEN** the new process acquires the instance lock and starts normally without a stale-lock error

### Requirement: Open requests are validated

The app SHALL validate an incoming path before playing it. The file MUST exist, be a regular file, and have a supported audio extension. Invalid, missing, or directory paths MUST be ignored without crashing; the running instance MUST remain usable.

**Verification:** unit test for `is_supported_audio_path` and handoff validation; integration test for the watcher.

#### Scenario: Missing path

- **WHEN** a forwarded path no longer exists
- **THEN** the request is ignored and the current playback continues

#### Scenario: Directory or unsupported extension

- **WHEN** the forwarded path is a directory or has an unsupported extension
- **THEN** the request is ignored without an error dialog

#### Scenario: Uppercase extension

- **WHEN** the path ends with `.MP3` or another supported extension in uppercase
- **THEN** the file is accepted

### Requirement: Windows executable uses the product icon

Release Windows builds SHALL embed `assets/favicon.ico` as the executable icon and MUST NOT show the default Dioxus icon in Explorer, the taskbar, or the window. The installer SHOULD use `favicon.ico` when the bundler supports ICO; if it does not, the executable icon MUST still be correct.

**Verification:** build the Windows release and check Explorer, taskbar, and window manually; verify the resource step is cfg-gated.

#### Scenario: Release executable icon

- **WHEN** a release Windows build is produced
- **THEN** the executable file icon is the Wave Player icon from `assets/favicon.ico`

#### Scenario: Running application icon

- **WHEN** the release application is running
- **THEN** the taskbar and window show the Wave Player icon rather than the default Dioxus icon

#### Scenario: Non-Windows build

- **WHEN** the project is built for macOS or Linux
- **THEN** the Windows resource step is skipped and the build succeeds

#### Scenario: Bundler without ICO support

- **WHEN** the installer bundler cannot use `favicon.ico`
- **THEN** the installer may keep the existing PNG icon while the executable icon remains correct
