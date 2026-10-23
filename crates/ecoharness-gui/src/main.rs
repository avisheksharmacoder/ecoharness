pub mod state;
pub mod view;

#[cfg(test)]
mod tests;

use ecoharness::OllamaConfig;
use state::{AppState, Message};
use std::env;

fn main() -> iced::Result {
    iced::application(
        || {
            let base_dir = env::temp_dir().join("harness2_gui_workspace");
            let probe_dir = env::temp_dir().join("harness2_gui_probes");

            let state = AppState::new(base_dir, probe_dir, OllamaConfig::default())
                .expect("Failed to initialize Harness² workspace");
            (state, iced::Task::done(Message::RefreshOllamaModels))
        },
        AppState::update,
        view::view,
    )
    .title("Harness² Studio — Rust + Ollama")
    .font(lucide_icons::LUCIDE_FONT_BYTES)
    .run()
}
