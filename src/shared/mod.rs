pub mod command_types;
pub mod event_types;
pub mod store_types;

use std::path::Path;

/// Версия приложения, берётся автоматически из `package.version` в Cargo.toml.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Audio extensions accepted by the file picker and desktop open requests.
pub const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "ogg", "m4a", "aac"];

/// Build number injected by CI through `WAVE_PLAYER_BUILD`.
pub const APP_BUILD: Option<&str> = option_env!("WAVE_PLAYER_BUILD");

/// Version label with the CI build number when available.
pub fn version_label() -> String {
    match APP_BUILD.filter(|build| !build.is_empty()) {
        Some(build) => format!("{APP_VERSION} (build {build})"),
        None => APP_VERSION.to_string(),
    }
}

/// Checks that the process can create and remove files in `directory`.
pub(crate) fn probe_directory_writable(directory: &Path) -> std::io::Result<()> {
    let probe = directory.join(".wave_player_write_probe");
    std::fs::write(&probe, b"probe")?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::probe_directory_writable;

    #[test]
    fn writable_probe_should_clean_up_after_itself() {
        let directory =
            std::env::temp_dir().join(format!("wave_player_write_probe_{}", std::process::id()));

        std::fs::create_dir_all(&directory).unwrap();

        assert!(probe_directory_writable(&directory).is_ok());
        assert!(!directory.join(".wave_player_write_probe").exists());

        let _ = std::fs::remove_dir_all(&directory);
    }
}
