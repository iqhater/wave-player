use dioxus::prelude::*;

/// User's theme preference.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

/// Global theme preference shared through the Dioxus context API.
///
/// Wrapped in a dedicated type (instead of a bare `Signal<Theme>`) so it can
/// never be confused with other signals in the context.
#[derive(Clone, Copy)]
pub struct ThemeContext(pub Signal<Theme>);

impl ThemeContext {
    pub fn get(&self) -> Theme {
        (self.0)()
    }

    pub fn set(&self, theme: Theme) {
        let mut signal = self.0;
        *signal.write() = theme;
    }

    pub fn toggle(&self) {
        let next = match self.get() {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        };
        self.set(next);
    }
}

/// Provides the [`ThemeContext`] to the whole component tree.
pub fn use_theme_provider() {
    let theme = use_signal(|| Theme::default());
    use_context_provider(|| ThemeContext(theme));
}

/// Reads the [`ThemeContext`]. Panics if called outside a provider.
pub fn use_theme() -> ThemeContext {
    use_context::<ThemeContext>()
}
