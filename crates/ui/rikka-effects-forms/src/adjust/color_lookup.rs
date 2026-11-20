use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_effects::adjust::color_lookup::ColorLookup;
use rikka_ui::{
    Element, Styled, TextStyled, View, WindowIdentifier, local_state, skip_view, spawn_blocking,
    text, tr, window_identifier,
};
use rikka_widgets::{banner, button};

#[derive(Clone, Debug, Default)]
struct FormState {
    trouble: Option<String>,
}

fn ask(parent: Option<WindowIdentifier>) -> Loaded {
    let tables = PATTERNS.iter().fold(
        Filter::named(tr!("effects.color_lookup.tables").to_string()),
        |filter, pattern| filter.pattern(*pattern),
    );
    let chooser = Chooser::open(tr!("effects.color_lookup.load_title").to_string())
        .parent(parent)
        .accept_label(tr!("effects.color_lookup.load_accept").to_string())
        .filter(tables);
    match chooser.run() {
        Ok(ChooserResponse::Files(paths)) => paths
            .first()
            .map(|path| ColorLookup::read(path).map_err(|error| error.to_string())),
        Ok(ChooserResponse::Cancelled) => None,
        Err(trouble) => Some(Err(trouble.to_string())),
    }
}

const PATTERNS: [&str; 4] = ["*.cube", "*.CUBE", "*.3dl", "*.3DL"];

type Loaded = Option<std::result::Result<ColorLookup, String>>;

impl Form for ColorLookup {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        let tuner = tuner.clone();
        local_state(FormState::default(), move |shown: FormState, show| {
            let shown_table = match tuner.current().name() {
                Some(name) => text(name.to_owned()).bold().fill_width().any(),
                None => rows::note(tr!("effects.color_lookup.none")),
            };
            let landing = tuner.commit(
                move |lookup: &mut ColorLookup,
                      loaded: std::result::Result<ColorLookup, String>| {
                    match loaded {
                        Ok(table) => {
                            *lookup = table;
                            show(FormState::default());
                        }
                        Err(reason) => {
                            *lookup = ColorLookup::default();
                            show(FormState {
                                trouble: Some(reason),
                            });
                        }
                    }
                },
            );
            let load = move || {
                let landing = landing.clone();
                let parent = window_identifier();
                spawn_blocking(
                    move || ask(parent),
                    move |host: &mut S, loaded: Loaded| match loaded {
                        Some(loaded) => landing(host, loaded),
                        None => skip_view(),
                    },
                );
            };
            let mut rows = vec![
                rows::labelled(tr!("effects.color_lookup.table"), shown_table),
                button(text(tr!("effects.color_lookup.load")))
                    .small()
                    .on_press(load)
                    .any(),
                rows::note(tr!("effects.color_lookup.formats")),
            ];
            if let Some(reason) = shown.trouble {
                rows.push(
                    banner(tr!(
                        "effects.color_lookup.unloaded",
                        reason = reason.as_str()
                    ))
                    .any(),
                );
            }
            rows::form_sheet(rows)
        })
    }
}
