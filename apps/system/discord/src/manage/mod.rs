mod bar;
mod dialogs;
mod moderation;
mod pick;
mod prompts;

use discord_core::{
    ChannelId, ChannelKind, InviteRequest, MfaChallenge, MfaMethod, PurgeSpan, RoleId, SpaceId,
    SpaceInvite, SpacePrivacy, TimeoutSpan, UserId,
};
use rikka_app::prelude::*;

use crate::chat::Chat;
pub(crate) use bar::unsaved_bar;
pub(crate) use moderation::timeout_label;
pub(crate) use pick::{Picked, pick_file, pick_icon, pick_picture};

pub(crate) type Retry = Box<dyn FnOnce(&mut Chat)>;

pub(crate) fn rgb_of(color: Color) -> u32 {
    let [red, green, blue, _] = color.to_rgba8();
    (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Dialog {
    Leave {
        key: String,
        space: SpaceId,
    },
    Privacy {
        key: String,
        space: SpaceId,
        privacy: SpacePrivacy,
    },
    Nickname {
        key: String,
        space: SpaceId,
        user: UserId,
    },
    CreateChannel {
        key: String,
        space: SpaceId,
        parent: Option<ChannelId>,
        kind: ChannelKind,
        private: bool,
    },
    Invite {
        key: String,
        channel: ChannelId,
        request: InviteRequest,
        invite: Option<SpaceInvite>,
    },
    Folder {
        key: String,
        folder: String,
        color: Option<u32>,
    },
    DeleteChannel {
        key: String,
        channel: ChannelId,
    },
    Kick {
        key: String,
        space: SpaceId,
        user: UserId,
    },
    Ban {
        key: String,
        space: SpaceId,
        user: UserId,
        purge: PurgeSpan,
    },
    Timeout {
        key: String,
        space: SpaceId,
        user: UserId,
        span: TimeoutSpan,
    },
    Roles {
        key: String,
        space: SpaceId,
        user: UserId,
    },
    Transfer {
        key: String,
        space: SpaceId,
        user: UserId,
    },
    DeleteSpace {
        key: String,
        space: SpaceId,
    },
    Prune {
        key: String,
        space: SpaceId,
        days: u8,
        roles: Vec<RoleId>,
        count: Option<u32>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Prompt {
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) accept: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MfaAsk {
    pub(crate) key: String,
    pub(crate) challenge: MfaChallenge,
    pub(crate) method: MfaMethod,
    pub(crate) sent: bool,
}

#[derive(State)]
pub(crate) struct Manage {
    dialog: Tracked<Option<Dialog>>,
    busy: Tracked<bool>,
    error_message: Tracked<Option<String>>,
    #[untracked]
    text: TextDocument,
    #[untracked]
    reason: TextDocument,
    filled: Tracked<bool>,
    prompt: Tracked<Option<Prompt>>,
    #[untracked]
    confirmed: Option<Retry>,
    mfa: Tracked<Option<MfaAsk>>,
    #[untracked]
    code: TextDocument,
    coded: Tracked<bool>,
    #[untracked]
    retries: Vec<Retry>,
    #[untracked]
    abandons: Vec<Retry>,
    #[untracked]
    resume_busy: bool,
    alert: Tracked<Option<String>>,
}

impl Manage {
    pub(crate) fn new() -> Manage {
        Manage {
            dialog: Tracked::new(None),
            busy: Tracked::new(false),
            error_message: Tracked::new(None),
            text: TextDocument::new(""),
            reason: TextDocument::new(""),
            filled: Tracked::new(false),
            prompt: Tracked::new(None),
            confirmed: None,
            mfa: Tracked::new(None),
            code: TextDocument::new(""),
            coded: Tracked::new(false),
            retries: Vec::new(),
            abandons: Vec::new(),
            resume_busy: false,
            alert: Tracked::new(None),
        }
    }

    pub(crate) fn dialog(&self) -> Option<&Dialog> {
        self.dialog.as_ref()
    }

    pub(crate) fn is_busy(&self) -> bool {
        *self.busy
    }

    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub(crate) fn text_field(&self) -> &TextDocument {
        &self.text
    }

    pub(crate) fn text(&self) -> String {
        self.text.text().trim().to_owned()
    }

    pub(crate) fn is_filled(&self) -> bool {
        *self.filled
    }

    pub(crate) fn reason_field(&self) -> &TextDocument {
        &self.reason
    }

    pub(crate) fn reason(&self) -> Option<String> {
        Some(self.reason.text().trim().to_owned()).filter(|reason| !reason.is_empty())
    }

    pub(crate) fn show(&mut self, dialog: Dialog, text: &str) {
        self.text.reset(text);
        self.reason.reset("");
        self.filled.set(!text.trim().is_empty());
        self.busy.set(false);
        self.error_message.set(None);
        self.dialog.set(Some(dialog));
    }

    pub(crate) fn retext(&mut self) {
        self.filled.set(!self.text().is_empty());
        self.error_message.set(None);
    }

    pub(crate) fn change(&mut self, change: impl FnOnce(&mut Dialog)) {
        if let Some(mut dialog) = (*self.dialog).clone() {
            change(&mut dialog);
            self.dialog.set(Some(dialog));
        }
    }

    pub(crate) fn change_invite(&mut self, change: impl FnOnce(&mut InviteRequest)) {
        self.change(|dialog| {
            if let Dialog::Invite {
                request, invite, ..
            } = dialog
            {
                change(request);
                *invite = None;
            }
        });
    }

    pub(crate) fn close(&mut self) {
        self.dialog.set(None);
        self.busy.set(false);
    }

    pub(crate) fn idle(&mut self) {
        self.busy.set(false);
    }

    pub(crate) fn clear_text(&mut self) {
        self.text.reset("");
        self.filled.set(false);
    }

    pub(crate) fn start(&mut self) {
        self.busy.set(true);
        self.error_message.set(None);
    }

    pub(crate) fn failed(&mut self, error_message: String) {
        self.busy.set(false);
        match self.dialog.is_some() {
            true => self.error_message.set(Some(error_message)),
            false => self.alert.set(Some(error_message)),
        }
    }

    pub(crate) fn prompt(&self) -> Option<&Prompt> {
        self.prompt.as_ref()
    }

    pub(crate) fn ask(&mut self, prompt: Prompt, confirmed: Retry) {
        self.prompt.set(Some(prompt));
        self.confirmed = Some(confirmed);
    }

    pub(crate) fn answer(&mut self, accepted: bool) -> Option<Retry> {
        self.prompt.set(None);
        let confirmed = self.confirmed.take();
        accepted.then_some(confirmed).flatten()
    }

    pub(crate) fn mfa(&self) -> Option<&MfaAsk> {
        self.mfa.as_ref()
    }

    pub(crate) fn code_field(&self) -> &TextDocument {
        &self.code
    }

    pub(crate) fn code(&self) -> String {
        self.code.text()
    }

    pub(crate) fn is_coded(&self) -> bool {
        *self.coded
    }

    pub(crate) fn recode(&mut self) {
        self.coded.set(!self.code.text().trim().is_empty());
        self.error_message.set(None);
    }

    pub(crate) fn ask_mfa(
        &mut self,
        key: String,
        challenge: MfaChallenge,
        retry: Retry,
        abandon: Retry,
    ) {
        self.retries.push(retry);
        self.abandons.push(abandon);
        if self.mfa.is_some() {
            return;
        }
        let method = challenge
            .methods
            .first()
            .copied()
            .unwrap_or(MfaMethod::Password);
        self.resume_busy = *self.busy;
        self.code.reset("");
        self.coded.set(false);
        self.busy.set(false);
        self.error_message.set(None);
        self.mfa.set(Some(MfaAsk {
            key,
            challenge,
            method,
            sent: false,
        }));
    }

    pub(crate) fn choose_mfa(&mut self, method: MfaMethod) {
        if let Some(mut ask) = (*self.mfa).clone() {
            ask.method = method;
            self.code.reset("");
            self.coded.set(false);
            self.error_message.set(None);
            self.mfa.set(Some(ask));
        }
    }

    pub(crate) fn sms_sent(&mut self) {
        self.busy.set(false);
        if let Some(mut ask) = (*self.mfa).clone() {
            ask.sent = true;
            self.mfa.set(Some(ask));
        }
    }

    pub(crate) fn mfa_failed(&mut self, error_message: String) {
        self.busy.set(false);
        self.error_message.set(Some(error_message));
    }

    pub(crate) fn finish_mfa(&mut self) -> Vec<Retry> {
        self.mfa.set(None);
        self.busy.set(self.resume_busy);
        self.abandons.clear();
        std::mem::take(&mut self.retries)
    }

    pub(crate) fn cancel_mfa(&mut self) -> Vec<Retry> {
        self.mfa.set(None);
        self.retries.clear();
        self.busy.set(false);
        std::mem::take(&mut self.abandons)
    }

    pub(crate) fn alert(&self) -> Option<&str> {
        self.alert.as_deref()
    }

    pub(crate) fn dismiss_alert(&mut self) {
        self.alert.set(None);
    }
}

impl std::fmt::Debug for Manage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Manage")
            .field("dialog", &*self.dialog)
            .field("busy", &*self.busy)
            .field("prompt", &*self.prompt)
            .field("mfa", &*self.mfa)
            .finish_non_exhaustive()
    }
}

pub(crate) fn overlays(chat: &Chat, stage: &'static str) -> Vec<Element> {
    let manage = chat.manage();
    let mut layers = Vec::new();
    if let Some(dialog) = manage.dialog().filter(|_| manage.mfa().is_none()) {
        layers.extend(dialogs::view(chat, dialog, stage));
    }
    if let Some(prompt) = manage.prompt() {
        layers.extend(prompts::prompt(prompt, stage));
    }
    if let Some(ask) = manage.mfa() {
        layers.extend(prompts::mfa(manage, ask, stage));
    }
    if let Some(alert) = manage.alert() {
        layers.extend(prompts::alert(alert, stage));
    }
    layers
}
