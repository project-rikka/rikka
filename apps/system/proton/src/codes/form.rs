use proton_codes::{Error, Kind, Setup, StoredCode};
use proton_session::{ErrorBanner, Failure};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::codes::Codes;
use crate::pane;

#[derive(Debug, State)]
pub(crate) struct Form {
    id: Option<String>,
    title: TextDocument,
    secret: TextDocument,
    issuer: TextDocument,
    note: TextDocument,
    setup: Setup,
    advanced: bool,
    error: ErrorBanner,
}

impl Form {
    pub(crate) fn new_id() -> Form {
        Form {
            id: None,
            title: TextDocument::new(""),
            secret: TextDocument::new(""),
            issuer: TextDocument::new(""),
            note: TextDocument::new(""),
            setup: Setup::default(),
            advanced: false,
            error: ErrorBanner::default(),
        }
    }

    pub(crate) fn from_code(entry: &StoredCode) -> Form {
        let mut form = Form {
            id: Some(entry.id().to_owned()),
            note: TextDocument::new(entry.note().unwrap_or_default()),
            ..Form::new_id()
        };
        form.fill(Setup::from_otp(entry.otp()));
        form
    }

    pub(crate) fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub(crate) fn setup(&self) -> Setup {
        Setup {
            title: self.title.text(),
            secret: self.secret.text(),
            issuer: self.issuer.text(),
            ..self.setup.clone()
        }
    }

    pub(crate) fn note(&self) -> Option<String> {
        let note = self.note.text();
        (!note.trim().is_empty()).then_some(note)
    }

    pub(crate) fn fill(&mut self, setup: Setup) {
        self.title.set_text(&setup.title);
        self.secret.set_text(&setup.secret);
        self.issuer.set_text(&setup.issuer);
        let usual = Setup::default();
        self.advanced = self.advanced
            || (setup.digits, setup.period, setup.algorithm)
                != (usual.digits, usual.period, usual.algorithm);
        self.setup = setup;
        self.error.clear();
    }

    pub(crate) fn took_secret(&mut self, secret: &str) {
        self.secret.set_text(secret);
        self.error.clear();
    }

    pub(crate) fn failed(&mut self, error: &Error) {
        self.error.set(Failure::new(
            match error {
                Error::Untitled => tr!("proton.codes.needs-title"),
                Error::NoSecret => tr!("proton.codes.needs-secret"),
                Error::Authenticator(_) => tr!("proton.codes.unreadable-secret"),
                _ => Label::from(error.to_string()),
            }
            .as_str(),
        ));
    }

    fn unfold(&mut self) {
        self.advanced = !self.advanced;
    }

    fn choose_kind(&mut self, at: usize) {
        if let Some(kind) = Kind::ALL.get(at) {
            self.setup.kind = *kind;
        }
    }

    fn choose_digits(&mut self, at: usize) {
        if let Some(digits) = self.setup.digits_offered().get(at) {
            self.setup.digits = *digits;
        }
    }

    fn choose_period(&mut self, at: usize) {
        if let Some(period) = self.setup.periods_offered().get(at) {
            self.setup.period = *period;
        }
    }

    fn choose_algorithm(&mut self, at: usize) {
        if let Some(algorithm) = Setup::ALGORITHMS.get(at) {
            self.setup.algorithm = *algorithm;
        }
    }
}

pub(crate) fn view(form: &Form) -> Element {
    let steam = form.setup.kind == Kind::Steam;

    column! {
        pane::toolbar(
            row! {
                text(match form.id.is_none() {
                    true => tr!("proton.codes.new"),
                    false => tr!("proton.codes.editing"),
                })
                .label()
                .semibold()
                .grow(1.0);
                button(icon(MaterialSymbol::CLOSE).extent(Length::px(15.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(|codes: &mut Codes| codes.close_form());
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width(),
        );
        rule();
        pane::error_banner(&form.error, |form: &mut Form| &mut form.error);
        scrollable(
            column! {
                button(pane::labelled(MaterialSymbol::CONTENT_PASTE, tr!("proton.codes.paste")))
                    .secondary()
                    .small()
                    .on_press(|codes: &mut Codes| codes.paste());
                pane::input(
                    tr!("proton.codes.title"),
                    text_field(&form.title)
                        .placeholder(tr!("proton.codes.title-hint"))
                        .autofocus(true)
                        .fill_width(),
                );
                pane::input(
                    tr!("proton.codes.secret"),
                    text_field(&form.secret)
                        .placeholder(tr!("proton.codes.secret"))
                        .fill_width(),
                );
                if steam {
                    kind(form);
                } else {
                    pane::input(
                        tr!("proton.codes.issuer"),
                        text_field(&form.issuer)
                            .placeholder(tr!("proton.codes.issuer"))
                            .fill_width(),
                    );
                    advanced(form);
                }
                pane::input(tr!("proton.codes.note"), pane::prose(&form.note, 60.0..=160.0));
                row! {
                    button(text(tr!("proton.codes.save")))
                        .primary()
                        .on_press(|codes: &mut Codes| codes.keep());
                    spacer().grow(1.0);
                }
                .fill_width();
            }
            .gap(Space::NORMAL)
            .padding(Space::ROOMY)
            .fill_width(),
        )
        .grow(1.0)
        .fill_width();
    }
    .fill_height()
    .build()
}

fn advanced(form: &Form) -> impl View {
    let setup = &form.setup;
    let digits = setup.digits_offered();
    let periods = setup.periods_offered();

    column! {
        button(
            row! {
                text(tr!("proton.codes.advanced")).caption().semibold().no_wrap();
                icon(match form.advanced {
                    true => MaterialSymbol::EXPAND_LESS,
                    false => MaterialSymbol::EXPAND_MORE,
                })
                .extent(Length::px(15.0));
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        )
        .ghost()
        .small()
        .on_press(|form: &mut Form| form.unfold());
        if form.advanced {
            row! {
                pane::input(
                    tr!("proton.codes.digits"),
                    combo(digits.iter().position(|value| *value == setup.digits))
                        .options(digits.iter().map(u8::to_string))
                        .control_size(ControlSize::Small)
                        .on_select(|form: &mut Form, at: usize| form.choose_digits(at))
                        .fill_width(),
                )
                .grow(1.0);
                pane::input(
                    tr!("proton.codes.period"),
                    combo(periods.iter().position(|value| *value == setup.period))
                        .options(periods.iter().map(|period| {
                            tr!("proton.codes.seconds", count = &period.to_string())
                        }))
                        .control_size(ControlSize::Small)
                        .on_select(|form: &mut Form, at: usize| form.choose_period(at))
                        .fill_width(),
                )
                .grow(1.0);
            }
            .gap(Space::NORMAL)
            .fill_width();
            pane::input(
                tr!("proton.codes.algorithm"),
                segmented(
                    Setup::ALGORITHMS
                        .iter()
                        .position(|value| *value == setup.algorithm)
                        .unwrap_or_default(),
                )
                .segments(Setup::ALGORITHMS.iter().map(|algorithm| algorithm.as_str()))
                .control_size(ControlSize::Small)
                .on_select(|form: &mut Form, at: usize| form.choose_algorithm(at)),
            );
            kind(form);
        }
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn kind(form: &Form) -> impl View {
    pane::input(
        tr!("proton.codes.kind"),
        segmented(
            Kind::ALL
                .iter()
                .position(|value| *value == form.setup.kind)
                .unwrap_or_default(),
        )
        .segments([tr!("proton.codes.totp"), tr!("proton.codes.steam")])
        .control_size(ControlSize::Small)
        .on_select(|form: &mut Form, at: usize| form.choose_kind(at)),
    )
}
