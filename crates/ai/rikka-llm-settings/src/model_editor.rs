use std::ops::RangeInclusive;

use crate::prelude::*;
use rikka_llm::{Effort, ModelInfo};
use rikka_llm_accounts::{AccountId, effort_label};
use rikka_widgets::{column, row};

use crate::llm_settings::LlmSettings;

const CONTEXT: RangeInclusive<f32> = 512.0..=10_000_000.0;
const OUTPUT: RangeInclusive<f32> = 16.0..=1_000_000.0;
const STEP: f32 = 1024.0;
const LEVELS_PER_ROW: usize = 4;

#[derive(Debug)]
pub(crate) struct ModelEditor {
    account: AccountId,
    id: String,
    name: TextDocument,
    context: u64,
    output: u32,
    tools: bool,
    vision: bool,
    efforts: Vec<Effort>,
}

impl ModelEditor {
    pub(crate) fn new(account: AccountId, model: &ModelInfo) -> ModelEditor {
        ModelEditor {
            account,
            id: model.id().to_owned(),
            name: TextDocument::new(model.name()),
            context: model.context(),
            output: model.output(),
            tools: model.tools(),
            vision: model.vision(),
            efforts: model.efforts().to_vec(),
        }
    }

    pub(crate) fn account(&self) -> AccountId {
        self.account
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn model_info(&self) -> ModelInfo {
        let name = self.name.text();
        let name = match name.trim() {
            "" => self.id.as_str(),
            named => named,
        };
        let mut efforts = self.efforts.clone();
        efforts.sort_unstable();
        efforts.dedup();
        ModelInfo::new(
            &self.id,
            name,
            self.context,
            self.output.min(context_cap(self.context)),
        )
        .with_tools(self.tools)
        .with_vision(self.vision)
        .with_efforts(efforts)
    }

    fn toggle(&mut self, effort: Effort, on: bool) {
        self.efforts.retain(|held| *held != effort);
        if on {
            self.efforts.push(effort);
        }
    }
}

fn context_cap(context: u64) -> u32 {
    u32::try_from(context).unwrap_or(u32::MAX)
}

pub(crate) fn view(editor: &ModelEditor) -> impl View {
    column! {
        text(editor.id.clone())
            .label()
            .medium()
            .family(Family::parse(font::JETBRAINS_MONO));
        field(
            tr!("llm-settings.model.name"),
            text_field(&editor.name)
                .placeholder(editor.id.clone())
                .control_size(ControlSize::Small)
                .on_change(|| {})
                .fill_width(),
        );
        field(
            tr!("llm-settings.model.context"),
            number_field(editor.context as f32)
                .range(CONTEXT)
                .step(STEP)
                .unit(tr!("llm-settings.model.tokens"))
                .control_size(ControlSize::Small)
                .on_commit(|settings: &mut LlmSettings, tokens: f32| {
                    if let Some(editor) = settings.accounts.models.editor_mut() {
                        editor.context = tokens as u64;
                    }
                }),
        );
        field(
            tr!("llm-settings.model.output"),
            number_field(editor.output as f32)
                .range(OUTPUT)
                .step(STEP)
                .unit(tr!("llm-settings.model.tokens"))
                .control_size(ControlSize::Small)
                .on_commit(|settings: &mut LlmSettings, tokens: f32| {
                    if let Some(editor) = settings.accounts.models.editor_mut() {
                        editor.output = tokens as u32;
                    }
                }),
        );
        field(
            tr!("llm-settings.model.tools"),
            switch(editor.tools)
                .control_size(ControlSize::Small)
                .on_toggle(|settings: &mut LlmSettings, on: bool| {
                    if let Some(editor) = settings.accounts.models.editor_mut() {
                        editor.tools = on;
                    }
                }),
        );
        field(
            tr!("llm-settings.model.vision"),
            switch(editor.vision)
                .control_size(ControlSize::Small)
                .on_toggle(|settings: &mut LlmSettings, on: bool| {
                    if let Some(editor) = settings.accounts.models.editor_mut() {
                        editor.vision = on;
                    }
                }),
        );
        field(tr!("llm-settings.model.reasoning"), levels(editor));
        row! {
            spacer().grow(1.0);
            button(tr!("llm-settings.model.cancel"))
                .ghost()
                .small()
                .on_press(|settings: &mut LlmSettings| settings.accounts.models.close_editor());
            button(tr!("llm-settings.model.save"))
                .primary()
                .small()
                .on_press(|settings: &mut LlmSettings| settings.accounts.models.save_editor(&mut settings.llm));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn levels(editor: &ModelEditor) -> impl View {
    column! {
        for shelf in Effort::ALL.chunks(LEVELS_PER_ROW) {
            row! {
                for effort in shelf.iter().copied() {
                    checkbox(editor.efforts.contains(&effort))
                        .label(text(effort_label(effort)).caption())
                        .control_size(ControlSize::Small)
                        .on_toggle(move |settings: &mut LlmSettings, on: bool| {
                            if let Some(editor) = settings.accounts.models.editor_mut() {
                                editor.toggle(effort, on);
                            }
                        });
                }
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center);
        }
    }
    .gap(Space::TIGHT)
}
