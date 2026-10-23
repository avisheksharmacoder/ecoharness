use crate::state::{
    AppState, ChatSender, DetailPaneView, ExecutionStatus, Message, UiIcon,
};
use ecoharness::{HarnessComponent, ModelRole, MountTarget};
use iced::widget::{
    button, checkbox, column, container, pick_list, row, rule, scrollable, slider, text,
    text_editor, text_input, toggler, Column, Row,
};
use iced::{Alignment, Element, Font, Length};

const LUCIDE_FONT: Font = Font::with_name("lucide");

fn horizontal_rule<'a>(height: impl Into<iced::Pixels>) -> iced::widget::Rule<'a> {
    rule::horizontal(height)
}

fn vertical_rule<'a>(width: impl Into<iced::Pixels>) -> iced::widget::Rule<'a> {
    rule::vertical(width)
}

fn icon_badge(action: UiIcon) -> iced::widget::Text<'static> {
    let glyph = char::from(AppState::icon_for(action));
    text(glyph.to_string()).font(LUCIDE_FONT)
}

/// Renders the 2-pane Iced interface (Left Master-Detail Sidebar + Right Chat/Plan Workbench).
pub fn view(state: &AppState) -> Element<'_, Message> {
    let left_pane = render_left_pane(state);
    let right_pane = render_right_pane(state);

    row![left_pane, vertical_rule(1), right_pane]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn render_left_pane(state: &AppState) -> Element<'_, Message> {
    // 1. Top Master List A: Chat Sessions
    let chat_header = row![
        icon_badge(UiIcon::ChatList),
        text("Chat Sessions").size(16),
        button(text("+ New")).on_press(Message::CreateNewChatSession),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let mut session_list = Column::new().spacing(4);
    for session in &state.sessions {
        let label = if session.id == state.active_session_id {
            format!("● {}", session.title)
        } else {
            format!("  {}", session.title)
        };
        session_list = session_list.push(
            button(text(label).size(14))
                .width(Length::Fill)
                .on_press(Message::SelectChatSession(session.id)),
        );
    }

    // 2. Top Master List B: Ollama Models
    let ollama_header = row![
        icon_badge(UiIcon::OllamaServer),
        text("Ollama Models").size(16),
        button(row![icon_badge(UiIcon::RefreshModels), text("Sync")].spacing(4))
            .on_press(Message::RefreshOllamaModels),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let mut model_list = Column::new().spacing(4);
    if state.ollama_models.is_empty() {
        model_list = model_list.push(
            text("No models loaded yet (click Sync or check Ollama)").size(12),
        );
    } else {
        for model in &state.ollama_models {
            let is_active = state.driver.config().executor_model == model.name
                || state.driver.config().editor_model == model.name;
            let is_selected = state.selected_ollama_model.as_deref() == Some(model.name.as_str());
            let label = if is_active {
                format!("★ {}", model.name)
            } else if is_selected {
                format!("▸ {}", model.name)
            } else {
                format!("  {}", model.name)
            };
            let item_row = row![
                button(text(label).size(13))
                    .width(Length::Fill)
                    .on_press(Message::SelectOllamaModel(model.name.clone())),
                button(text(if is_active { "Active" } else { "Use" }).size(11))
                    .on_press(Message::SelectActiveModel(model.name.clone())),
            ]
            .spacing(4)
            .align_y(Alignment::Center);

            model_list = model_list.push(item_row);
        }
    }

    // 3. Bottom Contextual Detail Sub-Pane
    let detail_pane = render_detail_subpane(state);

    let left_column = column![
        chat_header,
        scrollable(session_list).height(Length::FillPortion(2)),
        horizontal_rule(1),
        ollama_header,
        scrollable(model_list).height(Length::FillPortion(2)),
        horizontal_rule(1),
        detail_pane,
    ]
    .spacing(10)
    .padding(12)
    .width(Length::FillPortion(2))
    .height(Length::Fill);

    container(left_column).into()
}

fn render_detail_subpane(state: &AppState) -> Element<'_, Message> {
    match &state.detail_view {
        DetailPaneView::HarnessInspector {
            session_id,
            selected_component,
        } => {
            let header = row![
                icon_badge(UiIcon::HarnessFiles),
                text(format!(
                    "8-File Harness Inspector (Session #{})",
                    session_id + 1
                ))
                .size(15),
            ]
            .spacing(6)
            .align_y(Alignment::Center);

            let mut row_top = Row::new().spacing(4);
            let mut row_bottom = Row::new().spacing(4);
            for (idx, comp) in HarnessComponent::all().iter().enumerate() {
                let btn = button(text(format!("{:?}", comp)).size(11))
                    .on_press(Message::SelectHarnessComponent(*comp));
                if idx < 4 {
                    row_top = row_top.push(btn);
                } else {
                    row_bottom = row_bottom.push(btn);
                }
            }

            let editor_controls = row![
                text(format!("File: {}", selected_component.relative_path())).size(12),
                button(row![icon_badge(UiIcon::SaveComponent), text("Save")].spacing(4))
                    .on_press(Message::SaveActiveHarnessComponent),
                button(text("Checkpoint v1").size(11))
                    .on_press(Message::SaveSnapshotVersion("v1".to_string())),
                button(text("Rollback v1").size(11))
                    .on_press(Message::RollbackSnapshotVersion("v1".to_string())),
            ]
            .spacing(6)
            .align_y(Alignment::Center);

            let multiline_editor = text_editor(&state.component_editor_content)
                .on_action(Message::ComponentEditorAction)
                .height(Length::Fill);

            column![
                header,
                text(state.inspector_summary()).size(11),
                row_top,
                row_bottom,
                editor_controls,
                multiline_editor,
            ]
            .spacing(6)
            .height(Length::FillPortion(4))
            .into()
        }

        DetailPaneView::OllamaModelDetail {
            model_name,
            assigned_role,
        } => {
            let cfg = state.driver.config();
            let header = row![
                icon_badge(UiIcon::OllamaServer),
                text(format!("Model Config: {}", model_name)).size(15),
            ]
            .spacing(6)
            .align_y(Alignment::Center);

            let endpoint_input = text_input("http://localhost:11434", &cfg.base_url)
                .on_input(Message::UpdateOllamaEndpoint)
                .size(13);

            let role_buttons = row![
                button(text("Set Executor").size(12)).on_press(Message::AssignModelRole(
                    model_name.clone(),
                    ModelRole::Executor
                )),
                button(text("Set Editor").size(12)).on_press(Message::AssignModelRole(
                    model_name.clone(),
                    ModelRole::Editor
                )),
                button(text("Set Both").size(12)).on_press(Message::AssignModelRole(
                    model_name.clone(),
                    ModelRole::Both
                )),
            ]
            .spacing(6);

            let temp_slider = row![
                text(format!("Temp: {:.2}", cfg.temperature)).size(12),
                slider(0.0..=2.0, cfg.temperature, Message::UpdateTemperature).step(0.05_f32),
            ]
            .spacing(8)
            .align_y(Alignment::Center);

            let json_toggle = row![
                checkbox(cfg.enforce_json_schema)
                    .on_toggle(Message::ToggleEnforceJsonSchema)
                    .size(13),
                text("Enforce Strict JSON Schema").size(13),
            ]
            .spacing(8)
            .align_y(Alignment::Center);

            let script_toggle = row![
                checkbox(state.script_execution_enabled)
                    .on_toggle(Message::ToggleScriptExecution)
                    .size(13),
                text("Enable Multi-Turn Scripts, Subagents & Plugins").size(13),
            ]
            .spacing(8)
            .align_y(Alignment::Center);

            column![
                header,
                text(format!("Active Role for Model: {:?}", assigned_role)).size(12),
                text(format!(
                    "Executor: {} | Editor: {}",
                    cfg.executor_model, cfg.editor_model
                ))
                .size(12),
                endpoint_input,
                role_buttons,
                temp_slider,
                json_toggle,
                script_toggle,
            ]
            .spacing(8)
            .height(Length::FillPortion(4))
            .into()
        }
    }
}

fn render_right_pane(state: &AppState) -> Element<'_, Message> {
    let cfg = state.driver.config();

    // 1. Top Header Bar with Plan Mode Toggle
    let status_badge = match &state.execution_status {
        ExecutionStatus::Idle => {
            if state.ppc_stage_history.is_empty() {
                "Ready".to_string()
            } else {
                format!("Ready (last PPC: {} stages)", state.ppc_stage_history.len())
            }
        }
        ExecutionStatus::RunningSinglePass => "Running Single-Pass Executor...".to_string(),
        ExecutionStatus::RunningPlanModePpc => {
            format!(
                "Plan Mode: Propose -> Probe -> Compose ({} stages logged)",
                state.ppc_stage_history.len()
            )
        }
        ExecutionStatus::Error(err) => format!("Error: {}", err),
    };

    let model_names: Vec<String> = state
        .ollama_models
        .iter()
        .map(|m| m.name.clone())
        .collect();

    let current_selected = if model_names.contains(&cfg.executor_model) {
        Some(cfg.executor_model.clone())
    } else {
        model_names.first().cloned()
    };

    let model_picker: Element<'_, Message> = if !model_names.is_empty() {
        row![
            text("Model:").size(13),
            pick_list(model_names, current_selected, Message::SelectActiveModel),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
    } else {
        text(format!("Model: {} (waiting for Ollama)", cfg.executor_model))
            .size(13)
            .into()
    };

    let top_bar = row![
        model_picker,
        text(format!("Status: {}", status_badge)).size(13),
        icon_badge(UiIcon::PlanMode),
        toggler(state.plan_mode_enabled)
            .label("Plan Mode (Recursive PPC)")
            .on_toggle(Message::TogglePlanMode),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    // 2. Center Chat & Plan Mode Probe Stream
    let mut message_stream = Column::new().spacing(10);
    for msg in &state.active_session().messages {
        let sender_tag = match msg.sender {
            ChatSender::User => "You",
            ChatSender::Assistant => "Harness² Agent",
            ChatSender::SystemNotice => "System",
        };

        let score_suffix = msg
            .score
            .map(|s| format!(" (score: {:.2})", s))
            .unwrap_or_default();

        let mut msg_col = column![
            text(format!("{}{}", sender_tag, score_suffix)).size(13),
            text(&msg.content).size(15),
        ]
        .spacing(4);

        if !msg.adopted_diffs.is_empty() {
            let comps: Vec<String> = msg
                .adopted_diffs
                .iter()
                .map(|d| format!("{:?} ({})", d.component, d.rationale))
                .collect();
            msg_col = msg_col.push(
                text(format!(
                    "✓ Plan Mode Adopted Harness Diffs: {}",
                    comps.join(", ")
                ))
                .size(12),
            );
        }

        if let Some(lesson) = &msg.lesson_learned {
            msg_col = msg_col
                .push(text(format!("★ EXPERIENCE.md Lesson: {}", lesson)).size(12));
        }

        message_stream = message_stream.push(container(msg_col).padding(8));
    }

    // 3. Bottom Add File & Composer Controls
    let inspected_component = match state.detail_view {
        DetailPaneView::HarnessInspector {
            selected_component, ..
        } => selected_component,
        DetailPaneView::OllamaModelDetail { .. } => HarnessComponent::ToolGuidance,
    };

    let target_label = match state.add_file_target {
        MountTarget::PromptContext => "Target: Prompt Context".to_string(),
        MountTarget::HarnessSlot(comp) => format!("Target: {:?} Slot", comp),
    };

    let file_bar = row![
        button(row![icon_badge(UiIcon::AddFile), text("Add File")].spacing(4))
            .on_press(Message::OpenNativeFileDialog),
        button(text("Route to Prompt").size(12))
            .on_press(Message::SelectAddFileTarget(MountTarget::PromptContext)),
        button(text(format!("Mount to {:?}", inspected_component)).size(12)).on_press(
            Message::SelectAddFileTarget(MountTarget::HarnessSlot(inspected_component))
        ),
        text(target_label).size(12),
        text(format!("Staged files: {}", state.staged_files.len())).size(12),
        button(text("Clear").size(12)).on_press(Message::ClearStagedFiles),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let prompt_bar = row![
        text_input(
            "Ask LFM2.5-350M or run a Plan Mode Propose-Probe-Compose task...",
            &state.prompt_input,
        )
        .on_input(Message::PromptInputChanged)
        .on_submit(Message::SendPrompt)
        .width(Length::Fill),
        button(row![icon_badge(UiIcon::SendPrompt), text("Send")].spacing(6))
            .on_press(Message::SendPrompt),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    column![
        top_bar,
        horizontal_rule(1),
        scrollable(message_stream).height(Length::Fill),
        horizontal_rule(1),
        file_bar,
        prompt_bar,
    ]
    .spacing(10)
    .padding(12)
    .width(Length::FillPortion(3))
    .height(Length::Fill)
    .into()
}
