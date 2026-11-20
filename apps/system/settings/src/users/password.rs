use rikka_app::prelude::*;
use rikka_pam::{PasswordError, Secret};
use rikka_secrets::{KeyringState, Secrets};
use rikka_widgets::{column, row};
use zeroize::Zeroizing;

use crate::notice::notice;
use crate::users::component::{Report, Users};

const FIELD: f32 = 260.0;

#[derive(Debug)]
pub(crate) struct PasswordForm {
    current: TextDocument,
    new: TextDocument,
    confirm: TextDocument,
    revealed: bool,
    changing: bool,
    failure: Option<Failure>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Failure {
    Incorrect,
    Rejected(Option<String>),
    Unavailable,
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Problem {
    Empty,
    Unchanged,
    Mismatch,
}

impl PasswordForm {
    fn new() -> PasswordForm {
        PasswordForm {
            current: TextDocument::new(""),
            new: TextDocument::new(""),
            confirm: TextDocument::new(""),
            revealed: false,
            changing: false,
            failure: None,
        }
    }

    fn problem(&self) -> Option<Problem> {
        if self.new.is_empty() {
            return Some(Problem::Empty);
        }
        let new = Zeroizing::new(self.new.text());
        if *new == *Zeroizing::new(self.current.text()) {
            return Some(Problem::Unchanged);
        }
        (*new != *Zeroizing::new(self.confirm.text())).then_some(Problem::Mismatch)
    }

    fn ready(&self, problem: Option<Problem>) -> bool {
        !self.changing && !self.current.is_empty() && problem.is_none()
    }

    pub(crate) fn is_changing(&self) -> bool {
        self.changing
    }

    fn wipe(&self) {
        for document in [&self.current, &self.new, &self.confirm] {
            document.reset("");
        }
    }
}

impl Failure {
    fn from_error(error: PasswordError) -> Failure {
        match error {
            PasswordError::Incorrect => Failure::Incorrect,
            PasswordError::Rejected { reason } => Failure::Rejected(reason),
            PasswordError::Unavailable => Failure::Unavailable,
            other => Failure::Failed(other.to_string()),
        }
    }

    fn message(&self) -> Label {
        match self {
            Failure::Incorrect => tr!("settings.users.password-incorrect"),
            Failure::Rejected(Some(reason)) => {
                tr!("settings.users.password-rejected-because", reason = reason)
            }
            Failure::Rejected(None) => tr!("settings.users.password-rejected"),
            Failure::Unavailable => tr!("settings.users.password-unavailable"),
            Failure::Failed(reason) => tr!("settings.users.password-failed", reason = reason),
        }
    }
}

impl Users {
    pub(crate) fn close_password(&mut self) {
        if let Some(form) = self.password.take() {
            form.wipe();
        }
    }

    fn open_password(&mut self) {
        self.report = None;
        self.password = Some(PasswordForm::new());
    }

    fn reveal_password(&mut self) {
        if let Some(form) = &mut self.password {
            form.revealed = !form.revealed;
        }
    }

    fn change_password(&mut self) {
        let Some(form) = &mut self.password else {
            return;
        };
        if !form.ready(form.problem()) {
            return;
        }
        form.changing = true;
        form.failure = None;
        let current = Secret::new(form.current.text());
        let new = Zeroizing::new(form.new.text());
        spawn_blocking(
            move || {
                let changed = rikka_pam::change_password(&current, &Secret::new(new.to_string()));
                (changed, new)
            },
            |users: &mut Users, (changed, new): (Result<(), PasswordError>, Zeroizing<String>)| {
                users.password_changed(changed, new);
            },
        );
    }

    fn password_changed(&mut self, changed: Result<(), PasswordError>, new: Zeroizing<String>) {
        match changed {
            Ok(()) => {
                self.close_password();
                self.report = Some(Report::PasswordChanged);
                spawn(
                    follow_keyring(new),
                    |users: &mut Users, followed: Result<(), String>| {
                        if let Err(reason) = followed {
                            users.report = Some(Report::KeyringBehind(reason));
                        }
                    },
                );
            }
            Err(error) => {
                let Some(form) = &mut self.password else {
                    return;
                };
                form.changing = false;
                let failure = Failure::from_error(error);
                if failure == Failure::Incorrect {
                    form.current.reset("");
                }
                form.failure = Some(failure);
            }
        }
    }
}

async fn follow_keyring(password: Zeroizing<String>) -> Result<(), String> {
    let followed = async {
        let keyring = Secrets::shared().await?.keyring();
        if keyring.status().await?.state != KeyringState::Absent {
            keyring.login(password.as_bytes()).await?;
        }
        Ok::<(), rikka_secrets::Error>(())
    };
    followed.await.map_err(|error| error.to_string())
}

pub(crate) fn view(form: Option<&PasswordForm>) -> impl View {
    section(
        tr!("settings.users.password"),
        match form {
            None => field(
                tr!("settings.users.password"),
                button(tr!("settings.users.change-password"))
                    .secondary()
                    .on_press(Users::open_password),
            )
            .build(),
            Some(form) => editing(form).build(),
        },
    )
}

fn editing(form: &PasswordForm) -> impl View {
    let problem = form.problem();
    let confirm_typed = !form.confirm.is_empty();
    let new_typed = !form.new.is_empty();
    let hint = match problem {
        Some(Problem::Unchanged) if new_typed => Some(tr!("settings.users.password-unchanged")),
        Some(Problem::Mismatch) if confirm_typed => Some(tr!("settings.users.password-mismatch")),
        _ => None,
    };
    column! {
        form.failure.as_ref().map(|failure| {
            notice(MaterialSymbol::ERROR, palette().danger, failure.message())
        });
        field(
            tr!("settings.users.current-password"),
            secret_field(form, &form.current)
                .autofocus(true)
                .error(form.failure == Some(Failure::Incorrect)),
        );
        field(
            tr!("settings.users.new-password"),
            secret_field(form, &form.new).error(new_typed && problem == Some(Problem::Unchanged)),
        );
        field(
            tr!("settings.users.confirm-password"),
            secret_field(form, &form.confirm)
                .error(confirm_typed && problem == Some(Problem::Mismatch))
                .on_submit(Users::change_password),
        );
        hint.map(|hint| text(hint).caption().color(palette().danger));
        row! {
            button(icon(match form.revealed {
                true => MaterialSymbol::VISIBILITY_OFF,
                false => MaterialSymbol::VISIBILITY,
            }))
            .ghost()
            .icon_only()
            .enabled(!form.changing)
            .on_press(Users::reveal_password)
            .tooltip(match form.revealed {
                true => tr!("settings.users.hide-passwords"),
                false => tr!("settings.users.show-passwords"),
            });
            spacer().grow(1.0);
            button(tr!("settings.users.cancel"))
                .secondary()
                .enabled(!form.changing)
                .on_press(Users::close_password);
            button(tr!("settings.users.change-password"))
                .primary()
                .loading(form.changing)
                .enabled(form.ready(problem))
                .on_press(Users::change_password);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn secret_field(form: &PasswordForm, document: &TextDocument) -> TextField {
    text_field(document)
        .secret(!form.revealed)
        .enabled(!form.changing)
        .on_change(|| {})
        .width(FIELD)
}
