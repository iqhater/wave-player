use crate::shared::store_types::ThemePreference;
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
pub fn use_theme_provider(initial_theme: Theme) {
    let theme = use_signal(|| initial_theme);
    use_context_provider(|| ThemeContext(theme));
}

/// Reads the [`ThemeContext`]. Panics if called outside a provider.
pub fn use_theme() -> ThemeContext {
    use_context::<ThemeContext>()
}

impl Theme {
    pub fn from_pref(pref: ThemePreference) -> Self {
        match pref {
            ThemePreference::Light => Theme::Light,
            ThemePreference::Dark => Theme::Dark,
        }
    }

    pub fn to_pref(self) -> ThemePreference {
        match self {
            Theme::Light => ThemePreference::Light,
            Theme::Dark => ThemePreference::Dark,
        }
    }
}
