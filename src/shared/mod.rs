pub mod command_types;
pub mod event_types;
pub mod store_types;

/// Версия приложения, берётся автоматически из `package.version` в Cargo.toml.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Build number injected by CI through `WAVE_PLAYER_BUILD`.
pub const APP_BUILD: Option<&str> = option_env!("WAVE_PLAYER_BUILD");

/// Version label with the CI build number when available.
pub fn version_label() -> String {
    match APP_BUILD.filter(|build| !build.is_empty()) {
        Some(build) => format!("{APP_VERSION} (build {build})"),
        None => APP_VERSION.to_string(),
    }
}
