use crate::prelude::*;
use rikka_llm_accounts::{
    Account, AccountId, Checkup, Endpoint, EndpointCredentials, Header, Listing, Wire,
};
use rikka_widgets::{column, row};

use crate::llm_settings::LlmSettings;
use crate::rows::asking;

const WIRES: [Wire; 2] = [Wire::Completions, Wire::Responses];

#[derive(Debug)]
pub(crate) struct EndpointForm {
    editing: Option<AccountId>,
    model: Option<String>,
    name: TextDocument,
    address: TextDocument,
    key: TextDocument,
    revealed: bool,
    wire: Wire,
    headers: Vec<HeaderRow>,
    problem: Option<Problem>,
    tests: u64,
    check: Check,
}

#[derive(Debug)]
struct HeaderRow {
    name: TextDocument,
    value: TextDocument,
}

#[derive(Debug)]
enum Problem {
    Unnamed,
    Address,
    Header(String),
}

#[derive(Debug)]
enum Check {
    Idle,
    Running(u64),
    Done(Result<Checkup, String>),
}

pub(crate) struct Filled {
    pub(crate) name: String,
    pub(crate) endpoint: Endpoint,
    pub(crate) credentials: EndpointCredentials,
}

impl EndpointForm {
    pub(crate) fn new() -> EndpointForm {
        EndpointForm {
            editing: None,
            model: None,
            name: TextDocument::new(""),
            address: TextDocument::new(""),
            key: TextDocument::new(""),
            revealed: false,
            wire: Wire::default(),
            headers: Vec::new(),
            problem: None,
            tests: 0,
            check: Check::Idle,
        }
    }

    pub(crate) fn editing(account: &Account) -> EndpointForm {
        let mut form = EndpointForm::new();
        form.editing = Some(account.id());
        form.model = Some(account.model())
            .filter(|model| !model.is_empty())
            .or_else(|| account.models().first().map(|model| model.id()))
            .map(str::to_owned);
        form.name.set_text(account.label());
        if let Some(endpoint) = account.endpoint() {
            form.address.set_text(endpoint.base());
            form.wire = endpoint.wire();
        }
        if let Some(credentials) = account.endpoint_credentials() {
            form.key.set_text(credentials.key());
            form.headers = credentials
                .headers()
                .iter()
                .map(|header| HeaderRow {
                    name: TextDocument::new(header.name()),
                    value: TextDocument::new(header.value()),
                })
                .collect();
        }
        form
    }

    pub(crate) fn edits(&self) -> Option<AccountId> {
        self.editing
    }

    pub(crate) fn filled(&mut self) -> Option<Filled> {
        match self.read() {
            Ok(filled) => {
                self.problem = None;
                Some(filled)
            }
            Err(problem) => {
                self.problem = Some(problem);
                None
            }
        }
    }

    fn read(&self) -> Result<Filled, Problem> {
        let name = self.name.text().trim().to_owned();
        if name.is_empty() {
            return Err(Problem::Unnamed);
        }
        let endpoint = Endpoint::new(&self.address.text())
            .map_err(|_| Problem::Address)?
            .with_wire(self.wire);
        let mut headers = Vec::new();
        for row in &self.headers {
            let (name, value) = (row.name.text(), row.value.text());
            if name.trim().is_empty() && value.trim().is_empty() {
                continue;
            }
            headers.push(Header::new(&name, &value).map_err(|_| Problem::Header(name))?);
        }
        Ok(Filled {
            name,
            endpoint,
            credentials: EndpointCredentials::new(self.key.text(), headers),
        })
    }

    fn test(&mut self) {
        let Some(filled) = self.filled() else {
            return;
        };
        self.tests += 1;
        let nth = self.tests;
        self.check = Check::Running(nth);
        let checking = filled
            .endpoint
            .check(&filled.credentials, self.model.as_deref());
        spawn(
            async move { checking.await.map_err(|error| error.to_string()) },
            move |settings: &mut LlmSettings, outcome: Result<Checkup, String>| {
                if let Some(form) = settings.accounts.form.as_mut() {
                    form.checked(nth, outcome);
                }
            },
        );
    }

    fn checked(&mut self, nth: u64, outcome: Result<Checkup, String>) {
        if matches!(self.check, Check::Running(running) if running == nth) {
            self.check = Check::Done(outcome);
        }
    }
}

fn open_form(settings: &mut LlmSettings) -> Option<&mut EndpointForm> {
    settings.accounts.form.as_mut()
}

pub(crate) fn view(form: &EndpointForm) -> impl View {
    let title = match form.editing {
        Some(_) => tr!(
            "llm-settings.endpoint.editing",
            name = form.name.text().as_str()
        ),
        None => tr!("llm-settings.endpoint.new"),
    };
    column! {
        text(title).label().medium();
        field(
            tr!("llm-settings.endpoint.name"),
            text_field(&form.name)
                .placeholder(tr!("llm-settings.endpoint.name-placeholder"))
                .error(matches!(form.problem, Some(Problem::Unnamed)))
                .control_size(ControlSize::Small)
                .on_change(|| {})
                .fill_width(),
        );
        field(
            tr!("llm-settings.endpoint.address"),
            text_field(&form.address)
                .placeholder(tr!("llm-settings.endpoint.address-placeholder"))
                .error(matches!(form.problem, Some(Problem::Address)))
                .control_size(ControlSize::Small)
                .on_change(|| {})
                .fill_width(),
        );
        field(tr!("llm-settings.endpoint.key"), key(form));
        field(
            tr!("llm-settings.endpoint.api"),
            segmented(WIRES.iter().position(|wire| *wire == form.wire).unwrap_or(0))
                .segments([
                    tr!("llm-settings.endpoint.completions"),
                    tr!("llm-settings.endpoint.responses"),
                ])
                .control_size(ControlSize::Small)
                .on_select(|settings: &mut LlmSettings, at: usize| {
                    if let (Some(form), Some(wire)) = (open_form(settings), WIRES.get(at)) {
                        form.wire = *wire;
                    }
                }),
        );
        headers(form);
        form.problem.as_ref().map(|problem| {
            text(match problem {
                Problem::Unnamed => tr!("llm-settings.endpoint.unnamed"),
                Problem::Address => tr!("llm-settings.endpoint.bad-address"),
                Problem::Header(name) => tr!("llm-settings.endpoint.bad-header", name = name.as_str()),
            })
            .caption()
            .color(TextColor::Warning)
        });
        checked(&form.check);
        actions(form);
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn key(form: &EndpointForm) -> impl View {
    let revealed = form.revealed;
    row! {
        text_field(&form.key)
            .secret(!revealed)
            .placeholder(tr!("llm-settings.endpoint.key-placeholder"))
            .control_size(ControlSize::Small)
            .on_change(|| {})
            .grow(1.0);
        button(
            icon(match revealed {
                true => MaterialSymbol::VISIBILITY_OFF,
                false => MaterialSymbol::VISIBILITY,
            })
            .extent(Length::px(18.0)),
        )
        .icon_only()
        .ghost()
        .on_press(move |settings: &mut LlmSettings| {
            if let Some(form) = open_form(settings) {
                form.revealed = !revealed;
            }
        });
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width()
}

fn headers(form: &EndpointForm) -> impl View {
    column! {
        row! {
            text(tr!("llm-settings.endpoint.headers")).caption().muted().grow(1.0);
            button(
                row! {
                    icon(MaterialSymbol::ADD).extent(Length::px(14.0));
                    text(tr!("llm-settings.endpoint.add-header")).caption();
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .ghost()
            .small()
            .on_press(|settings: &mut LlmSettings| {
                if let Some(form) = open_form(settings) {
                    form.headers.push(HeaderRow {
                        name: TextDocument::new(""),
                        value: TextDocument::new(""),
                    });
                }
            });
        }
        .align_items(Align::Center)
        .fill_width();
        for (at, header) in form.headers.iter().enumerate() {
            row! {
                text_field(&header.name)
                    .placeholder(tr!("llm-settings.endpoint.header-name"))
                    .control_size(ControlSize::Small)
                    .on_change(|| {})
                    .grow(1.0);
                text_field(&header.value)
                    .placeholder(tr!("llm-settings.endpoint.header-value"))
                    .control_size(ControlSize::Small)
                    .on_change(|| {})
                    .grow(2.0);
                button(icon(MaterialSymbol::CLOSE).extent(Length::px(16.0)))
                    .icon_only()
                    .ghost()
                    .on_press(move |settings: &mut LlmSettings| {
                        if let Some(form) = open_form(settings)
                            && at < form.headers.len()
                        {
                            form.headers.remove(at);
                        }
                    });
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .fill_width();
        }
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn checked(check: &Check) -> Option<Element> {
    let line = match check {
        Check::Idle => return None,
        Check::Running(_) => return Some(asking(tr!("llm-settings.endpoint.testing")).build()),
        Check::Done(Err(reason)) => {
            return Some(
                text(tr!(
                    "llm-settings.endpoint.failed",
                    reason = reason.as_str()
                ))
                .caption()
                .color(TextColor::Warning)
                .build(),
            );
        }
        Check::Done(Ok(checkup)) => {
            let reached = match checkup.listing() {
                Listing::Models(models) => {
                    tr!("llm-settings.endpoint.listed", count = models.len() as i64)
                }
                Listing::Unlisted => tr!("llm-settings.endpoint.unlisted"),
            };
            match checkup.answered() {
                Some(model) => format!(
                    "{reached} · {}",
                    tr!("llm-settings.endpoint.answered", model = model)
                ),
                None => reached.to_string(),
            }
        }
    };
    Some(text(line).caption().color(TextColor::Accent).build())
}

fn actions(form: &EndpointForm) -> impl View {
    let running = matches!(form.check, Check::Running(_));
    row! {
        button(tr!("llm-settings.endpoint.test"))
            .secondary()
            .small()
            .enabled(!running)
            .on_press(|settings: &mut LlmSettings| {
                if let Some(form) = open_form(settings) {
                    form.test();
                }
            });
        spacer().grow(1.0);
        button(tr!("llm-settings.cancel"))
            .ghost()
            .small()
            .on_press(|settings: &mut LlmSettings| settings.accounts.close_form());
        button(match form.editing {
            Some(_) => tr!("llm-settings.endpoint.save"),
            None => tr!("llm-settings.endpoint.add"),
        })
        .primary()
        .small()
        .on_press(|settings: &mut LlmSettings| settings.accounts.save_form(&mut settings.llm));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}
