use std::collections::BTreeSet;

use crate::prelude::*;
use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use rikka_format::format_count;
use rikka_llm::ModelInfo;
use rikka_llm_accounts::{Account, AccountId, Listing, Offered};
use rikka_widgets::{column, row};

use crate::llm_settings::LlmSettings;
use crate::model_editor::{self, ModelEditor};
use crate::rows::{asking, muted};
use crate::wording;

const PICKER: f32 = 280.0;

const SHOWN: usize = 40;

#[derive(Debug)]
pub(crate) struct Models {
    pub(super) open: Option<AccountId>,
    search: TextDocument,
    typed: TextDocument,
    offered: Option<Catalogue>,
    fetching: Option<AccountId>,
    error: Option<String>,
    editor: Option<ModelEditor>,
}

#[derive(Debug)]
struct Catalogue {
    account: AccountId,
    models: Vec<ModelInfo>,
    unlisted: bool,
}

#[derive(Debug)]
pub(super) struct Fetched {
    models: Vec<ModelInfo>,
    unlisted: bool,
}

impl Models {
    pub(crate) fn new() -> Models {
        Models {
            open: None,
            search: TextDocument::new(""),
            typed: TextDocument::new(""),
            offered: None,
            fetching: None,
            error: None,
            editor: None,
        }
    }

    pub(super) fn open(&mut self, account: &Account) {
        let id = account.id();
        self.open = Some(id);
        self.search.set_text("");
        self.typed.set_text("");
        if self.fetching == Some(id)
            || self
                .offered
                .as_ref()
                .is_some_and(|offered| offered.account == id)
        {
            return;
        }
        let Some(asking) = fetch(account) else {
            return;
        };
        self.fetching = Some(id);
        self.error = None;
        spawn(
            asking,
            move |settings: &mut LlmSettings, fetched: Result<Fetched, String>| {
                settings.accounts.models.models_fetched(id, fetched);
            },
        );
    }

    pub(super) fn close(&mut self) {
        self.open = None;
        self.editor = None;
    }

    pub(super) fn forget(&mut self, account: AccountId) {
        if self
            .offered
            .as_ref()
            .is_some_and(|offered| offered.account == account)
        {
            self.offered = None;
        }
        if self.open == Some(account) {
            self.close();
        }
    }

    fn models_fetched(&mut self, account: AccountId, fetched: Result<Fetched, String>) {
        if self.fetching != Some(account) {
            return;
        }
        self.fetching = None;
        match fetched {
            Ok(Fetched { models, unlisted }) => {
                self.offered = Some(Catalogue {
                    account,
                    models,
                    unlisted,
                });
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn offered(&self, account: AccountId) -> Option<&Catalogue> {
        self.offered
            .as_ref()
            .filter(|offered| offered.account == account)
    }

    fn pick(&mut self, account: &Account, id: &str, llm: &mut rikka_llm_accounts::Llm) {
        let Some(model) = self
            .offered(account.id())
            .and_then(|offered| offered.models.iter().find(|model| model.id() == id))
            .cloned()
        else {
            return;
        };
        match account.endpoint() {
            Some(_) => self.editor = Some(ModelEditor::new(account.id(), &model)),
            None => llm.add_model(account.id(), model),
        }
    }

    fn pick_typed(&mut self, account: AccountId) {
        let typed = self.typed.text();
        let id = typed.trim();
        if id.is_empty() {
            return;
        }
        let model = self
            .offered(account)
            .and_then(|offered| offered.models.iter().find(|model| model.id() == id))
            .cloned()
            .unwrap_or_else(|| Offered::unlisted(id).model_info());
        self.editor = Some(ModelEditor::new(account, &model));
    }

    fn edit(&mut self, account: AccountId, model: &ModelInfo) {
        self.editor = Some(ModelEditor::new(account, model));
    }

    pub(super) fn editor_mut(&mut self) -> Option<&mut ModelEditor> {
        self.editor.as_mut()
    }

    pub(super) fn close_editor(&mut self) {
        self.editor = None;
    }

    pub(super) fn save_editor(&mut self, llm: &mut rikka_llm_accounts::Llm) {
        let Some(editor) = self.editor.take() else {
            return;
        };
        let account = editor.account();
        llm.add_model(account, editor.model_info());
        if llm
            .account(account)
            .is_some_and(|held| held.model().is_empty())
        {
            llm.set_model(account, editor.id());
        }
        self.typed.set_text("");
    }

    fn matching(&self, account: &Account) -> (Vec<&ModelInfo>, usize) {
        let query = self.search.text();
        let needle = query.trim();
        let added: BTreeSet<&str> = account.models().iter().map(ModelInfo::id).collect();

        let mut matched = 0;
        let shown = self
            .offered(account.id())
            .map_or(&[] as &[ModelInfo], |offered| offered.models.as_slice())
            .iter()
            .filter(|model| !added.contains(model.id()))
            .filter(|model| named(model.id(), needle) || named(model.name(), needle))
            .inspect(|_| matched += 1)
            .take(SHOWN)
            .collect();

        (shown, matched)
    }
}

fn fetch(account: &Account) -> Option<BoxFuture<'static, Result<Fetched, String>>> {
    if let (Some(endpoint), Some(credentials)) =
        (account.endpoint(), account.endpoint_credentials())
    {
        let listing = endpoint.models(credentials);
        return Some(
            async move {
                match listing.await.map_err(|error| error.to_string())? {
                    Listing::Models(models) => Ok(Fetched {
                        models: models.iter().map(Offered::model_info).collect(),
                        unlisted: false,
                    }),
                    Listing::Unlisted => Ok(Fetched {
                        models: Vec::new(),
                        unlisted: true,
                    }),
                }
            }
            .boxed(),
        );
    }
    let backend = account.backend()?;
    Some(
        async move {
            let models = backend.offered().await.map_err(|error| error.to_string())?;
            Ok(Fetched {
                models: models.into_iter().filter(ModelInfo::tools).collect(),
                unlisted: false,
            })
        }
        .boxed(),
    )
}

pub(crate) fn view(models: &Models, account: &Account) -> impl View {
    let id = account.id();
    let open = models.open == Some(id);
    let editing = models
        .editor
        .as_ref()
        .filter(|editor| editor.account() == id);

    column! {
        heading(id, open);
        if account.models().is_empty() {
            muted(tr!("llm-settings.models-none"));
        } else {
            added_models(account);
        }
        if let Some(editor) = editing {
            model_editor::view(editor);
        } else if open {
            picker(models, account);
        }
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn picker(models: &Models, account: &Account) -> impl View {
    let id = account.id();
    let (shown, matched) = models.matching(account);
    let picked: Vec<String> = shown.iter().map(|model| model.id().to_owned()).collect();
    let unlisted = models.offered(id).is_some_and(|offered| offered.unlisted);
    let endpoint = account.endpoint().is_some();

    column! {
        if !unlisted {
            text_field(&models.search)
                .placeholder(tr!("llm-settings.models-search"))
                .control_size(ControlSize::Small)
                .on_change(|| {})
                .fill_width();
        }
        if let Some(error) = &models.error {
            text(tr!(
                "llm-settings.models-unavailable",
                provider = account.label(),
                reason = error.as_str()
            ))
            .caption()
            .color(TextColor::Warning);
        } else if models.fetching == Some(id) {
            asking(tr!("llm-settings.models-asking"));
        } else if unlisted {
            muted(tr!("llm-settings.model.unlisted"));
        } else if shown.is_empty() {
            muted(tr!("llm-settings.models-nothing"));
        } else {
            list(shown.iter().map(|model| offer(model)))
                .selected(None)
                .control_size(ControlSize::Small)
                .on_select(move |settings: &mut LlmSettings, index: usize| {
                    let Some(model) = picked.get(index) else {
                        return skip_view();
                    };
                    let Some(account) = settings.llm.account(id).cloned() else {
                        return skip_view();
                    };
                    settings.accounts.models.pick(&account, model, &mut settings.llm);
                })
                .height(PICKER)
                .fill_width();
            if matched > shown.len() {
                muted(tr!(
                    "llm-settings.models-more",
                    count = (matched - shown.len()) as i64
                ));
            }
        }
        if endpoint {
            by_id(models, id);
        }
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn by_id(models: &Models, account: AccountId) -> impl View {
    row! {
        text_field(&models.typed)
            .placeholder(tr!("llm-settings.model.id-placeholder"))
            .control_size(ControlSize::Small)
            .on_change(|| {})
            .on_submit(move |settings: &mut LlmSettings| settings.accounts.models.pick_typed(account))
            .grow(1.0);
        button(tr!("llm-settings.model.add"))
            .secondary()
            .small()
            .on_press(move |settings: &mut LlmSettings| settings.accounts.models.pick_typed(account));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn heading(account: AccountId, open: bool) -> impl View {
    row! {
        text(tr!("llm-settings.models")).label().medium().grow(1.0);
        if open {
            button(tr!("llm-settings.models-done"))
                .ghost()
                .small()
                .on_press(|settings: &mut LlmSettings| settings.accounts.models.close());
        } else {
            button(
                row! {
                    icon(MaterialSymbol::ADD).extent(Length::px(16.0));
                    text(tr!("llm-settings.models-add")).caption();
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .secondary()
            .small()
            .on_press(move |settings: &mut LlmSettings| {
                if let Some(held) = settings.llm.account(account).cloned() {
                    settings.accounts.models.open(&held);
                }
            });
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn added_models(account: &Account) -> impl View {
    let id = account.id();
    let editable = account.endpoint().is_some();
    let held: Vec<ModelInfo> = account.models().to_vec();
    list(account.models().iter().map(|model| {
        let removed = model.id().to_owned();
        list_item(model.name()).secondary(traits(model)).on_action(
            MaterialSymbol::CLOSE,
            move |settings: &mut LlmSettings| {
                settings.llm.remove_model(id, &removed);
            },
        )
    }))
    .selected(None)
    .control_size(ControlSize::Small)
    .on_select(
        move |settings: &mut LlmSettings, index: usize| match held.get(index) {
            Some(model) if editable => settings.accounts.models.edit(id, model),
            _ => skip_view(),
        },
    )
    .fill_width()
}

fn offer(model: &ModelInfo) -> ListItem {
    let row = list_item(model.name()).secondary(traits(model));

    match model.price() {
        Some(price) => row.trailing(wording::price(price)),
        None => row,
    }
}

fn traits(model: &ModelInfo) -> String {
    let mut parts = vec![model.id().to_owned()];
    if model.context() > 0 {
        parts.push(format_count(model.context()));
    }
    if model.vision() {
        parts.push(tr!("llm-settings.models-vision").to_string());
    }
    if !model.tools() {
        parts.push(tr!("llm-settings.model.no-tools").to_string());
    }
    parts.join(" · ")
}

fn named(name: &str, needle: &str) -> bool {
    needle.is_empty()
        || (name.len() >= needle.len()
            && name
                .as_bytes()
                .windows(needle.len())
                .any(|window| window.eq_ignore_ascii_case(needle.as_bytes())))
}
