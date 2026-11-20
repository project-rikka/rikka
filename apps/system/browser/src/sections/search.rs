use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::sections::engine_form::{EngineForm, Problem};
use crate::shell::Shell;

const CHOICE: f32 = 220.0;
const FIELD: f32 = 320.0;

pub(crate) fn view(shell: &Shell) -> Element {
    let browser = shell.browser();
    let engines = browser.search_engines();
    let chosen = engines.default_engine().id().to_owned();
    let listed: Vec<(String, String)> = engines
        .iter()
        .map(|engine| (engine.id().to_owned(), engine.name().to_owned()))
        .collect();
    let at = listed.iter().position(|(id, _)| *id == chosen);
    let ids: Vec<String> = listed.iter().map(|(id, _)| id.clone()).collect();
    let suggestions = browser.preferences().search_suggestions();
    let rows = engines.iter().map(|engine| {
        let id = engine.id().to_owned();
        let custom = !engine.is_builtin();
        let is_default = id == chosen;
        let editing = id.clone();
        let removing = id.clone();
        let defaulting = id.clone();
        row! {
            column! {
                text(engine.name().to_owned()).label().medium().no_wrap();
                text(engine.keyword().to_owned()).caption().muted().no_wrap();
            }
            .grow(1.0)
            .min_width(0.0);
            is_default.then(|| text(tr!("browser.search.default")).caption().muted());
            (!is_default).then(|| {
                button(text(tr!("browser.search.make-default")))
                    .ghost()
                    .small()
                    .on_press(move |shell: &mut Shell| shell.browser().set_default_engine(&defaulting))
            });
            custom.then(|| {
                button(text(tr!("browser.search.edit")))
                    .ghost()
                    .small()
                    .on_press(move |shell: &mut Shell| shell.edit_engine(Some(&editing)))
            });
            custom.then(|| {
                button(text(tr!("browser.search.remove")))
                    .ghost()
                    .small()
                    .on_press(move |shell: &mut Shell| shell.remove_engine(&removing))
            });
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .build()
        .keyed(id)
    });
    let editor = match shell.sections.engine.editing() {
        Some(_) => form(&shell.sections.engine),
        None => row! {
            button(text(tr!("browser.search.add")))
                .secondary()
                .small()
                .on_press(|shell: &mut Shell| shell.edit_engine(None));
        }
        .any(),
    };
    page([
        section(
            tr!("browser.search.title"),
            column! {
                field(
                    tr!("browser.search.engine"),
                    combo(at)
                        .options(listed.into_iter().map(|(_, name)| name))
                        .on_select(move |shell: &mut Shell, at: usize| {
                            if let Some(id) = ids.get(at) {
                                shell.browser().set_default_engine(id);
                            }
                        })
                        .width(CHOICE),
                );
                field(
                    tr!("browser.search.suggestions"),
                    switch(suggestions).on_toggle(|shell: &mut Shell, on: bool| {
                        shell.browser().preferences().set_search_suggestions(on);
                    }),
                );
            }
            .gap(Space::NORMAL)
            .fill_width(),
        ),
        section(
            tr!("browser.search.engines"),
            column! {
                column_of(rows).gap(Space::TIGHT).fill_width();
                editor;
            }
            .gap(Space::NORMAL)
            .fill_width(),
        ),
    ])
    .any()
}

fn form(engine: &EngineForm) -> Element {
    let problem = engine.problem();
    let entry = |document: &TextDocument, label: Label, wrong: bool| {
        field(
            label,
            text_field(document)
                .control_size(ControlSize::Small)
                .error(wrong)
                .on_submit(|shell: &mut Shell| shell.save_engine())
                .width(FIELD),
        )
    };
    let trouble = problem.map(|problem| {
        let find_summary = match problem {
            Problem::Name => tr!("browser.search.problem-name"),
            Problem::Keyword => tr!("browser.search.problem-keyword"),
            Problem::KeywordTaken => tr!("browser.search.problem-keyword-taken"),
            Problem::Address => tr!("browser.search.problem-address"),
        };
        text(find_summary).caption().color(palette().danger)
    });
    column! {
        entry(&engine.name, tr!("browser.search.name"), problem == Some(Problem::Name));
        entry(
            &engine.keyword,
            tr!("browser.search.keyword"),
            matches!(problem, Some(Problem::Keyword | Problem::KeywordTaken)),
        );
        entry(&engine.url, tr!("browser.search.address"), problem == Some(Problem::Address));
        entry(&engine.suggestions, tr!("browser.search.suggestion-address"), false);
        trouble;
        row! {
            spacer().grow(1.0);
            button(text(tr!("browser.search.cancel")))
                .ghost()
                .small()
                .on_press(|shell: &mut Shell| shell.sections.engine.cancel());
            button(text(tr!("browser.search.save")))
                .primary()
                .small()
                .on_press(|shell: &mut Shell| shell.save_engine());
        }
        .gap(Space::TIGHT)
        .fill_width();
    }
    .gap(Space::TIGHT)
    .fill_width()
    .any()
}

impl Shell {
    fn edit_engine(&mut self, id: Option<&str>) {
        let custom_engines = self.browser().preferences().custom_engines();
        let chosen = id.and_then(|id| custom_engines.iter().find(|engine| engine.id == id));
        self.sections.engine.begin(chosen);
    }

    fn save_engine(&mut self) {
        let browser = std::rc::Rc::clone(self.browser());
        let custom_engines = browser.preferences().custom_engines();
        let engines = browser.search_engines();
        if let Some(next) = self.sections.engine.finish(&custom_engines, &engines) {
            browser.save_custom_engines(&next);
        }
    }

    fn remove_engine(&mut self, id: &str) {
        let browser = self.browser();
        let mut custom_engines = browser.preferences().custom_engines();
        custom_engines.retain(|engine| engine.id != id);
        browser.save_custom_engines(&custom_engines);
        if self.browser().search_engines().get(id).is_none()
            && self.browser().preferences().search_engine() == id
        {
            self.browser().set_default_engine("google");
        }
    }
}
