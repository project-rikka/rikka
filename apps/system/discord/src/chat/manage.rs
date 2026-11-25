use std::future::Future;

use discord_client::Client;
use discord_core::{
    ChannelDraft, ChannelId, ChannelKind, InviteRequest, MemberEdit, MfaMethod, PRUNE_DAYS,
    PrivacyChange, Prune, PurgeSpan, RoleId, SpaceId, TimeoutSpan, UserId,
};
use rikka_app::prelude::*;

use super::Chat;
use crate::manage::{Dialog, Manage, Prompt, Retry};

const MILLIS: i64 = 1000;

impl Chat {
    pub(crate) fn manage(&self) -> &Manage {
        &self.manage
    }

    pub(crate) fn act<T, F, Fut, D>(&mut self, key: &str, task: F, done: D)
    where
        T: Send + 'static,
        F: Fn(Client) -> Fut + Clone + 'static,
        Fut: Future<Output = discord_core::Result<T>> + Send + 'static,
        D: Fn(&mut Chat, T) + Clone + 'static,
    {
        self.attempt(key, task, move |chat: &mut Chat, value: Option<T>| {
            if let Some(value) = value {
                done(chat, value);
            }
        });
    }

    pub(crate) fn attempt<T, F, Fut, D>(&mut self, key: &str, task: F, settled: D)
    where
        T: Send + 'static,
        F: Fn(Client) -> Fut + Clone + 'static,
        Fut: Future<Output = discord_core::Result<T>> + Send + 'static,
        D: Fn(&mut Chat, Option<T>) + Clone + 'static,
    {
        let Some(client) = self.linked(key).map(|linked| linked.client().clone()) else {
            settled(self, None);
            return;
        };
        let key = key.to_owned();
        let again = task.clone();
        spawn(
            task(client),
            move |chat: &mut Chat, result: discord_core::Result<T>| match result {
                Ok(value) => settled(chat, Some(value)),
                Err(discord_core::Error::Mfa(challenge)) => {
                    let (owner, again, retried) = (key.clone(), again.clone(), settled.clone());
                    let abandoned = settled.clone();
                    let retry: Retry =
                        Box::new(move |chat: &mut Chat| chat.attempt(&owner, again, retried));
                    let abandon: Retry = Box::new(move |chat: &mut Chat| abandoned(chat, None));
                    chat.manage.ask_mfa(key.clone(), challenge, retry, abandon);
                }
                Err(error) => {
                    chat.manage.failed(error.to_string());
                    settled(chat, None);
                }
            },
        );
    }

    pub(crate) fn cancel_mfa(&mut self) {
        for abandon in self.manage.cancel_mfa() {
            abandon(self);
        }
    }

    pub(crate) fn manage_failed(&mut self, error_message: String) {
        self.manage.failed(error_message);
    }

    pub(crate) fn confirm(&mut self, prompt: Prompt, action: impl FnOnce(&mut Chat) + 'static) {
        self.manage.ask(prompt, Box::new(action));
    }

    pub(crate) fn answer_prompt(&mut self, accepted: bool) {
        if let Some(action) = self.manage.answer(accepted) {
            action(self);
        }
    }

    pub(crate) fn send_mfa_sms(&mut self) {
        let Some(ask) = self.manage.mfa().cloned() else {
            return;
        };
        let Some(client) = self.linked(&ask.key).map(|linked| linked.client().clone()) else {
            return;
        };
        self.manage.start();
        spawn(
            async move { client.send_mfa_sms(&ask.challenge).await },
            |manage: &mut Manage, sent: discord_core::Result<()>| match sent {
                Ok(()) => manage.sms_sent(),
                Err(error) => manage.mfa_failed(error.to_string()),
            },
        );
    }

    pub(crate) fn submit_mfa(&mut self) {
        let Some(ask) = self.manage.mfa().cloned() else {
            return;
        };
        let code = self.manage.code();
        if code.trim().is_empty() || self.manage.is_busy() {
            return;
        }
        let Some(client) = self.linked(&ask.key).map(|linked| linked.client().clone()) else {
            return;
        };
        self.manage.start();
        let method: MfaMethod = ask.method;
        spawn(
            async move { client.finish_mfa(&ask.challenge, method, &code).await },
            |chat: &mut Chat, finished: discord_core::Result<()>| match finished {
                Ok(()) => {
                    for retry in chat.manage.finish_mfa() {
                        retry(chat);
                    }
                }
                Err(error) => chat.manage.mfa_failed(error.to_string()),
            },
        );
    }

    fn open_dialog(&mut self, dialog: Dialog, text: &str) {
        self.close_overlays();
        self.manage.show(dialog, text);
    }

    pub(crate) fn ask_leave(&mut self, key: &str, space: &SpaceId) {
        self.open_dialog(
            Dialog::Leave {
                key: key.to_owned(),
                space: *space,
            },
            "",
        );
    }

    pub(crate) fn open_privacy(&mut self, key: &str, space: &SpaceId) {
        let Some(privacy) = self
            .linked(key)
            .map(|linked| linked.client().space_privacy(space))
        else {
            return;
        };
        match privacy {
            Ok(privacy) => self.open_dialog(
                Dialog::Privacy {
                    key: key.to_owned(),
                    space: *space,
                    privacy,
                },
                "",
            ),
            Err(error) => self.manage.failed(error.to_string()),
        }
    }

    pub(crate) fn change_privacy(&mut self, change: PrivacyChange) {
        let Some(Dialog::Privacy { key, space, .. }) = self.manage.dialog().cloned() else {
            return;
        };
        self.manage.change(|dialog| {
            if let Dialog::Privacy { privacy, .. } = dialog {
                match change {
                    PrivacyChange::Directs(on) => privacy.directs = on,
                    PrivacyChange::MessageRequests(on) => privacy.message_requests = on,
                    PrivacyChange::Activity(on) => privacy.activity = on,
                    PrivacyChange::Joining(on) => privacy.joining = on,
                }
            }
        });
        let owner = key.clone();
        self.attempt(
            &key,
            move |client| async move { client.change_privacy(&space, change).await },
            move |chat: &mut Chat, changed: Option<()>| {
                if changed.is_some() {
                    return;
                }
                let Some(Ok(current)) = chat
                    .linked(&owner)
                    .map(|linked| linked.client().space_privacy(&space))
                else {
                    return;
                };
                chat.manage.change(|dialog| {
                    if let Dialog::Privacy {
                        space: shown,
                        privacy,
                        ..
                    } = dialog
                        && *shown == space
                    {
                        *privacy = current;
                    }
                });
            },
        );
    }

    pub(crate) fn ask_nickname(&mut self, key: &str, space: &SpaceId, user: &UserId) {
        let current = self
            .linked(key)
            .and_then(|linked| linked.member(space, user))
            .and_then(|member| member.nickname.clone())
            .unwrap_or_default();
        self.open_dialog(
            Dialog::Nickname {
                key: key.to_owned(),
                space: *space,
                user: *user,
            },
            &current,
        );
    }

    pub(crate) fn reset_nickname(&mut self) {
        self.manage.clear_text();
        self.submit_dialog();
    }

    pub(crate) fn ask_create_channel(
        &mut self,
        key: &str,
        space: &SpaceId,
        parent: Option<ChannelId>,
        kind: ChannelKind,
    ) {
        self.open_dialog(
            Dialog::CreateChannel {
                key: key.to_owned(),
                space: *space,
                parent,
                kind,
                private: false,
            },
            "",
        );
    }

    pub(crate) fn open_invite(&mut self, key: &str, channel: &ChannelId) {
        self.open_dialog(
            Dialog::Invite {
                key: key.to_owned(),
                channel: *channel,
                request: InviteRequest::default(),
                invite: None,
            },
            "",
        );
        self.submit_dialog();
    }

    pub(crate) fn ask_folder(&mut self, key: &str, folder: &str) {
        let Some(found) = self.linked(key).and_then(|linked| linked.folder(folder)) else {
            return;
        };
        let (name, color) = (found.name.clone(), found.color);
        self.open_dialog(
            Dialog::Folder {
                key: key.to_owned(),
                folder: folder.to_owned(),
                color,
            },
            name.as_deref().unwrap_or_default(),
        );
    }

    pub(crate) fn ask_delete_channel(&mut self, key: &str, channel: &ChannelId) {
        self.open_dialog(
            Dialog::DeleteChannel {
                key: key.to_owned(),
                channel: *channel,
            },
            "",
        );
    }

    pub(crate) fn ask_kick(&mut self, key: &str, space: &SpaceId, user: &UserId) {
        self.open_dialog(
            Dialog::Kick {
                key: key.to_owned(),
                space: *space,
                user: *user,
            },
            "",
        );
    }

    pub(crate) fn ask_ban(&mut self, key: &str, space: &SpaceId, user: &UserId) {
        self.open_dialog(
            Dialog::Ban {
                key: key.to_owned(),
                space: *space,
                user: *user,
                purge: PurgeSpan::None,
            },
            "",
        );
    }

    pub(crate) fn ask_timeout(&mut self, key: &str, space: &SpaceId, user: &UserId) {
        self.open_dialog(
            Dialog::Timeout {
                key: key.to_owned(),
                space: *space,
                user: *user,
                span: TimeoutSpan::Hour,
            },
            "",
        );
    }

    pub(crate) fn ask_member_roles(&mut self, key: &str, space: &SpaceId, user: &UserId) {
        self.open_dialog(
            Dialog::Roles {
                key: key.to_owned(),
                space: *space,
                user: *user,
            },
            "",
        );
    }

    pub(crate) fn ask_transfer(&mut self, key: &str, space: &SpaceId, user: &UserId) {
        self.open_dialog(
            Dialog::Transfer {
                key: key.to_owned(),
                space: *space,
                user: *user,
            },
            "",
        );
    }

    pub(crate) fn ask_delete_space(&mut self, key: &str, space: &SpaceId) {
        self.open_dialog(
            Dialog::DeleteSpace {
                key: key.to_owned(),
                space: *space,
            },
            "",
        );
    }

    pub(crate) fn ask_prune(&mut self, key: &str, space: &SpaceId) {
        self.open_dialog(
            Dialog::Prune {
                key: key.to_owned(),
                space: *space,
                days: PRUNE_DAYS[0],
                roles: Vec::new(),
                count: None,
            },
            "",
        );
        self.count_prune();
    }

    pub(crate) fn change_prune(&mut self, change: impl FnOnce(&mut u8, &mut Vec<RoleId>)) {
        self.manage.change(|dialog| {
            if let Dialog::Prune {
                days, roles, count, ..
            } = dialog
            {
                change(days, roles);
                *count = None;
            }
        });
        self.count_prune();
    }

    fn count_prune(&mut self) {
        let Some(Dialog::Prune {
            key,
            space,
            days,
            roles,
            ..
        }) = self.manage.dialog().cloned()
        else {
            return;
        };
        let prune = Prune { days, roles };
        let asked = prune.clone();
        self.act(
            &key,
            move |client| {
                let prune = prune.clone();
                async move { client.prune_count(&space, &prune).await }
            },
            move |chat: &mut Chat, pruned: u32| {
                let asked = asked.clone();
                chat.manage.change(|dialog| {
                    if let Dialog::Prune {
                        days, roles, count, ..
                    } = dialog
                        && *days == asked.days
                        && *roles == asked.roles
                    {
                        *count = Some(pruned);
                    }
                });
            },
        );
    }

    pub(crate) fn toggle_member_role(&mut self, role: &RoleId, granted: bool) {
        let Some(Dialog::Roles { key, space, user }) = self.manage.dialog().cloned() else {
            return;
        };
        let role = *role;
        self.act(
            &key,
            move |client| async move { client.grant_role(&space, &user, &role, granted).await },
            |_: &mut Chat, ()| {},
        );
    }

    pub(crate) fn submit_dialog(&mut self) {
        let Some(dialog) = self.manage.dialog().cloned() else {
            return;
        };
        if self.manage.is_busy() {
            return;
        }
        let text = self.manage.text();
        let reason = self.manage.reason();
        self.manage.start();
        let closed = |chat: &mut Chat, ()| chat.manage.close();
        match dialog {
            Dialog::Leave { key, space } => self.act(
                &key,
                move |client| async move { client.leave_space(&space).await },
                closed,
            ),
            Dialog::Privacy { .. } | Dialog::Roles { .. } => self.manage.close(),
            Dialog::Nickname { key, space, user } => {
                let nickname = Some(text).filter(|text| !text.is_empty());
                let me = self.linked(&key).is_some_and(|linked| linked.is_me(&user));
                self.act(
                    &key,
                    move |client| {
                        let nickname = nickname.clone();
                        async move {
                            match me {
                                true => client.set_nickname(&space, nickname).await,
                                false => {
                                    client
                                        .edit_member(
                                            &space,
                                            &user,
                                            vec![MemberEdit::Nickname(nickname)],
                                            None,
                                        )
                                        .await
                                }
                            }
                        }
                    },
                    closed,
                );
            }
            Dialog::CreateChannel {
                key,
                space,
                parent,
                kind,
                private,
            } => {
                let draft = ChannelDraft {
                    name: text,
                    kind,
                    parent,
                    private,
                };
                let opened = key.clone();
                self.act(
                    &key,
                    move |client| {
                        let draft = draft.clone();
                        async move { client.create_channel(&space, draft).await }
                    },
                    move |chat: &mut Chat, channel: ChannelId| {
                        chat.manage.close();
                        if kind != ChannelKind::Category
                            && chat.current_key() == Some(opened.as_str())
                        {
                            chat.open_channel(&channel);
                        }
                    },
                );
            }
            Dialog::Invite {
                key,
                channel,
                request,
                ..
            } => self.act(
                &key,
                move |client| async move { client.create_invite(&channel, request).await },
                |chat: &mut Chat, created: discord_core::SpaceInvite| {
                    chat.manage.change(|dialog| {
                        if let Dialog::Invite { invite, .. } = dialog {
                            *invite = Some(created);
                        }
                    });
                    chat.manage.idle();
                    chat.refresh_space_invites();
                    chat.refresh_channel_invites();
                },
            ),
            Dialog::Folder { key, folder, color } => {
                let name = Some(text).filter(|text| !text.is_empty());
                self.act(
                    &key,
                    move |client| {
                        let (folder, name) = (folder.clone(), name.clone());
                        async move { client.edit_folder(&folder, name, color).await }
                    },
                    closed,
                );
            }
            Dialog::DeleteChannel { key, channel } => self.act(
                &key,
                move |client| async move { client.delete_channel(&channel).await },
                move |chat: &mut Chat, ()| {
                    chat.manage.close();
                    chat.forget_channel_admin(&channel);
                },
            ),
            Dialog::Kick { key, space, user } => self.act(
                &key,
                move |client| {
                    let reason = reason.clone();
                    async move { client.kick(&space, &user, reason).await }
                },
                closed,
            ),
            Dialog::Ban {
                key,
                space,
                user,
                purge,
            } => self.act(
                &key,
                move |client| {
                    let reason = reason.clone();
                    async move { client.ban(&space, &user, purge, reason).await }
                },
                closed,
            ),
            Dialog::Timeout {
                key,
                space,
                user,
                span,
            } => {
                let until = now_ms() + span.seconds() * MILLIS;
                self.act(
                    &key,
                    move |client| {
                        let reason = reason.clone();
                        async move {
                            client
                                .edit_member(
                                    &space,
                                    &user,
                                    vec![MemberEdit::Timeout(Some(until))],
                                    reason,
                                )
                                .await
                        }
                    },
                    closed,
                );
            }
            Dialog::Transfer { key, space, user } => self.act(
                &key,
                move |client| async move { client.transfer_space(&space, &user).await },
                closed,
            ),
            Dialog::DeleteSpace { key, space } => self.act(
                &key,
                move |client| async move { client.delete_space(&space).await },
                |chat: &mut Chat, ()| {
                    chat.manage.close();
                    chat.close_space_admin();
                },
            ),
            Dialog::Prune {
                key,
                space,
                days,
                roles,
                ..
            } => {
                let prune = Prune { days, roles };
                self.act(
                    &key,
                    move |client| {
                        let (prune, reason) = (prune.clone(), reason.clone());
                        async move { client.prune(&space, &prune, reason).await }
                    },
                    closed_with,
                );
            }
        }
    }

    pub(crate) fn remove_timeout(&mut self, key: &str, space: &SpaceId, user: &UserId) {
        let (space, user) = (*space, *user);
        self.act(
            key,
            move |client| async move {
                client
                    .edit_member(&space, &user, vec![MemberEdit::Timeout(None)], None)
                    .await
            },
            |_: &mut Chat, ()| {},
        );
    }
}

fn closed_with(chat: &mut Chat, _: u32) {
    chat.manage.close();
}

pub(crate) fn now_ms() -> i64 {
    jiff::Timestamp::now().as_millisecond()
}
