use proton_mailbox::{Answering, Attaching, Draft};
use proton_session::{ErrorBanner, Failure};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_proton_account::AddressId;
use rikka_proton_client::Session;
use rikka_proton_mail::MessageId;
use rikka_widgets::{column, row};

use crate::mail::Mail;
use crate::workspace::{WorkspaceId, landing};
use crate::{pane, wording};

#[derive(Debug, State)]
pub(crate) struct Compose {
    addresses: Vec<(AddressId, String)>,
    from: usize,
    to: TextDocument,
    cc: TextDocument,
    bcc: TextDocument,
    subject: TextDocument,
    body: TextDocument,
    answering: Option<Answering>,
    id: Option<MessageId>,
    files: Vec<Attaching>,
    copies: bool,
    busy: bool,
    error: ErrorBanner,
    workspace_id: WorkspaceId,
}

impl Compose {
    pub(crate) fn fresh(session: &Session, workspace_id: WorkspaceId) -> Compose {
        Compose::from_draft(Draft::blank(None), session, workspace_id)
    }

    pub(crate) fn from_draft(
        draft: Draft,
        session: &Session,
        workspace_id: WorkspaceId,
    ) -> Compose {
        let addresses: Vec<(AddressId, String)> = session
            .keys()
            .sending_addresses()
            .map(|address| (address.id().clone(), address.email().to_owned()))
            .collect();
        let from = draft
            .from
            .as_ref()
            .and_then(|sender| addresses.iter().position(|(id, _)| id == sender))
            .unwrap_or_default();

        Compose {
            addresses,
            from,
            to: TextDocument::new(&draft.to),
            cc: TextDocument::new(&draft.cc),
            bcc: TextDocument::new(&draft.bcc),
            subject: TextDocument::new(&draft.subject),
            body: TextDocument::new(&draft.body),
            copies: !draft.cc.is_empty() || !draft.bcc.is_empty(),
            answering: draft.answering,
            id: draft.id,
            files: Vec::new(),
            busy: false,
            error: ErrorBanner::default(),
            workspace_id,
        }
    }

    pub(crate) fn choose_from(&mut self, at: usize) {
        self.from = at.min(self.addresses.len().saturating_sub(1));
    }

    pub(crate) fn show_copies(&mut self) {
        self.copies = !self.copies;
    }

    pub(crate) fn attach(&mut self) {
        let parent = window_identifier();
        spawn_blocking(
            move || {
                Chooser::open(tr!("proton.mail.attach").as_str())
                    .parent(parent)
                    .multiple(true)
                    .run()
                    .map(read)
                    .unwrap_or_default()
            },
            landing(
                self.workspace_id,
                |mail: &mut Mail, files: Vec<Attaching>| {
                    mail.attached(files);
                },
            ),
        );
    }

    pub(crate) fn took(&mut self, files: Vec<Attaching>) {
        self.files.extend(files);
    }

    pub(crate) fn drop_file(&mut self, at: usize) {
        if at < self.files.len() {
            self.files.remove(at);
        }
    }

    pub(crate) fn draft_id(&self) -> Option<MessageId> {
        self.id.clone()
    }

    pub(crate) fn draft(&self) -> Draft {
        Draft {
            id: self.id.clone(),
            from: self.addresses.get(self.from).map(|(id, _)| id.clone()),
            to: self.to.text(),
            cc: self.cc.text(),
            bcc: self.bcc.text(),
            subject: self.subject.text(),
            body: self.body.text(),
            answering: self.answering.clone(),
            files: self.files.clone(),
        }
    }

    pub(crate) fn working(&mut self) {
        self.busy = true;
        self.error.clear();
    }

    pub(crate) fn failed(&mut self, failure: Failure) {
        self.busy = false;
        self.error.set(failure);
    }
}

fn read(chosen: ChooserResponse) -> Vec<Attaching> {
    let ChooserResponse::Files(paths) = chosen else {
        return Vec::new();
    };

    paths
        .into_iter()
        .filter_map(|path| {
            let bytes = std::fs::read(&path).ok()?;
            let name = path.file_name()?.to_string_lossy().into_owned();
            let media_type = mime_guess::from_path(&path)
                .first_or_octet_stream()
                .to_string();
            Some(Attaching::new(name, media_type, bytes))
        })
        .collect()
}

pub(crate) fn view(compose: &Compose) -> impl View {
    let named: Vec<Label> = compose
        .addresses
        .iter()
        .map(|(_, email)| Label::from(email.as_str()))
        .collect();

    column! {
        heading(compose);
        rule();
        pane::error_banner(&compose.error, |compose: &mut Compose| &mut compose.error);
        fields(compose, named);
        rule();
        text_editor(&compose.body)
            .fill_width()
            .grow(1.0);
        files(compose);
        rule();
        footer();
    }
    .fill()
}

fn heading(compose: &Compose) -> impl View {
    pane::toolbar(
        row! {
            text(match compose.answering.is_some() {
                true => tr!("proton.mail.answering"),
                false => tr!("proton.mail.new-message"),
            })
            .label()
            .semibold()
            .grow(1.0);
            if compose.id.is_some() {
                button(icon(MaterialSymbol::DELETE).extent(Length::px(15.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(|mail: &mut Mail| mail.discard_draft());
            }
            button(text(tr!("proton.mail.keep")))
                .ghost()
                .small()
                .enabled(!compose.busy)
                .on_press(|mail: &mut Mail| mail.keep_draft());
            button(
                row! {
                    icon(MaterialSymbol::SEND).extent(Length::px(15.0));
                    text(tr!("proton.mail.send")).caption().no_wrap();
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .primary()
            .small()
            .loading(compose.busy)
            .on_press(|mail: &mut Mail| mail.send_draft());
            button(icon(MaterialSymbol::CLOSE).extent(Length::px(15.0)))
                .ghost()
                .small()
                .icon_only()
                .on_press(|mail: &mut Mail| mail.close_compose());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
}

fn fields(compose: &Compose, named: Vec<Label>) -> impl View {
    column! {
        row! {
            text(tr!("proton.mail.from")).caption().muted().width(56.0);
            combo(compose.from)
                .group(tr!("proton.mail.from"), named)
                .control_size(ControlSize::Small)
                .on_select(|compose: &mut Compose, at: usize| compose.choose_from(at))
                .grow(1.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        row! {
            text(tr!("proton.mail.to")).caption().muted().width(56.0);
            text_field(&compose.to)
                .placeholder(tr!("proton.mail.to-hint"))
                .control_size(ControlSize::Small)
                .grow(1.0);
            button(text(tr!("proton.mail.copies")))
                .ghost()
                .small()
                .on_press(|compose: &mut Compose| compose.show_copies());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        if compose.copies {
            row! {
                text(tr!("proton.mail.cc")).caption().muted().width(56.0);
                text_field(&compose.cc).control_size(ControlSize::Small).grow(1.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            row! {
                text(tr!("proton.mail.bcc")).caption().muted().width(56.0);
                text_field(&compose.bcc).control_size(ControlSize::Small).grow(1.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        row! {
            text(tr!("proton.mail.subject")).caption().muted().width(56.0);
            text_field(&compose.subject)
                .placeholder(tr!("proton.mail.subject-hint"))
                .control_size(ControlSize::Small)
                .grow(1.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::SNUG)
    .padding(Space::NORMAL)
    .fill_width()
}

fn files(compose: &Compose) -> Option<Element> {
    if compose.files.is_empty() {
        return None;
    }

    Some(
        column_of(compose.files.iter().enumerate().map(|(at, file)| {
            row! {
                icon(MaterialSymbol::ATTACH_FILE)
                    .extent(Length::px(14.0))
                    .color(palette().text_muted);
                text(file.name.clone()).caption().no_wrap().grow(1.0);
                text(wording::size(i64::try_from(file.size()).unwrap_or_default()))
                    .caption()
                    .muted();
                button(icon(MaterialSymbol::CLOSE).extent(Length::px(13.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(move |compose: &mut Compose| compose.drop_file(at));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width()
            .build()
        }))
        .gap(Space::TIGHT)
        .padding(Space::NORMAL)
        .fill_width()
        .build(),
    )
}

fn footer() -> impl View {
    container(
        row! {
            button(
                row! {
                    icon(MaterialSymbol::ATTACH_FILE).extent(Length::px(15.0));
                    text(tr!("proton.mail.attach")).caption().no_wrap();
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .ghost()
            .small()
            .on_press(|compose: &mut Compose| compose.attach());
            spacer().grow(1.0);
        }
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(12.0, 8.0))
    .fill_width()
}
