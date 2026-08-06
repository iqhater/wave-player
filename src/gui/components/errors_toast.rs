use crate::AudioError;
use crate::PlayerContext;
use dioxus::prelude::*;
use futures_util::StreamExt;

const STYLES_CSS: Asset = asset!("/src/gui/components/errors_toast.css");

#[component]
pub fn ErrorsToast() -> Element {
    let ctx = use_context::<PlayerContext>();
    let mut current_error = use_signal(|| None::<AudioError>);

    // Listen errors events from audio engine
    use_future(move || {
        let mut error_rx = ctx.error_rx.read().clone();
        async move {
            while let Some(error) = error_rx.next().await {
                current_error.set(Some(error));
            }
        }
    });

    rsx! {
        document::Stylesheet { href: STYLES_CSS }

        div { class: "toast-errors-wrapper",

            // Всплывающее уведомление (Toast), которое рендерится только при наличии ошибки
            if let Some(error) = (current_error)() {
                div {
                    class: "toast-error-notification",

                    div { class: "error-title", "⚠ Ошибка воспроизведения" }

                    // Переводим техническую ошибку Rust на понятный пользователю язык
                    div { class: "error-message",
                        match error {
                            AudioError::FileNotFound(path) => format!("Файл не найден по пути: {}", path),
                            AudioError::PermissionDenied => "Нет доступа к памяти устройства. Проверьте разрешения в настройках.".to_string(),
                            AudioError::UnsupportedFormat => "Этот формат аудио не поддерживается плеером.".to_string(),
                            AudioError::SeekFailed => "Ошибка перемотки! Перемотка не поддерживается для данного источника.".to_string(),
                            AudioError::FileCorrupted => "Файл поврежден или имеет неверную структуру.".to_string(),
                            AudioError::DeviceInitializationFailed => "Не удалось инициализировать аудиоустройство.".to_string(),
                            AudioError::DeviceLost => "Аудиоустройство было отключено (проверьте наушники).".to_string(),
                        }
                    }

                    // Кнопка закрытия уведомления
                    button {
                        class: "error-close-button",
                        onclick: move |_| current_error.set(None),
                        "Закрыть"
                    }
                }
            }
        }
    }
}
