pub mod command_types;
pub mod event_types;

/// Версия приложения, берётся автоматически из `package.version` в Cargo.toml.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
