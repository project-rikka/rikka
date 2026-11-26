mod alerts;
mod calls;
mod configure;
mod linking;
mod manage;
mod me;
mod menus;
mod nav;
mod people;
mod screens;
mod signing;
mod spaces;

pub(crate) use manage::now_ms;

use std::collections::HashMap;
use std::sync::Arc;

use discord_client::Discord;
use discord_core::{SavedAccount, SpaceId, UserId};
use discord_preferences::{Appearance, Preferences};
use discord_voice::{Krisp, KrispState};
use rikka_account_book::Accounts;
use rikka_app::prelude::*;

use crate::account::LinkedAccount;
use crate::channel_admin::ChannelAdmin;
use crate::conversation::Conversation;
use crate::friends::Friends;
use crate::manage::Manage;
use crate::member_menu::{MemberMenu, MemberTarget};
use crate::menus::Menus;
use crate::notices::Notices;
use crate::profile::Profiles;
use crate::screen::Screens;
use crate::settings::Settings;
use crate::sidebar::Sidebar;
use crate::signing::Signing;
use crate::space_admin::SpaceAdmin;
use crate::space_sheet::SpaceSheet;
use crate::switcher::Switcher;
use crate::voice::Calls;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    SignIn,
    Connecting,
    Desk,
}

#[derive(State)]
pub(crate) struct Chat {
    #[untracked]
    sender: Sender<Chat>,
    #[untracked]
    discord: Arc<Discord>,
    #[untracked]
    krisp: Krisp,
    krisp_state: Tracked<KrispState>,
    accounts: Tracked<Accounts<SavedAccount>>,
    preferences: Tracked<Preferences>,
    appearance: Tracked<Appearance>,
    #[untracked]
    links: Vec<LinkedAccount>,
    joined: Revision,
    current: Tracked<Option<String>>,
    pending_account: Tracked<Option<String>>,
    error_message: Tracked<Option<String>>,
    connection_error: Tracked<HashMap<String, String>>,
    #[untracked]
    notices: Notices,
    #[untracked]
    focused: bool,
    #[state]
    signing: Signing,
    #[state]
    sidebar: Sidebar,
    #[state]
    conversation: Conversation,
    #[state]
    calls: Calls,
    #[state]
    screens: Screens,
    #[state]
    profiles: Profiles,
    #[state]
    member_menu: MemberMenu,
    #[state]
    friends: Friends,
    #[state]
    switcher: Switcher,
    #[state]
    space_sheet: SpaceSheet,
    #[state]
    settings: Settings,
    #[state]
    menus: Menus,
    #[state]
    manage: Manage,
    #[state]
    space_admin: SpaceAdmin,
    #[state]
    channel_admin: ChannelAdmin,
}

impl Chat {
    pub(crate) fn new(
        sender: Sender<Chat>,
        discord: Arc<Discord>,
        krisp: Krisp,
        accounts: Accounts<SavedAccount>,
        preferences: Preferences,
    ) -> Chat {
        Chat {
            sender,
            discord,
            krisp_state: Tracked::new(krisp.state()),
            krisp,
            accounts: Tracked::new(accounts),
            appearance: Tracked::new(preferences.appearance()),
            preferences: Tracked::new(preferences),
            links: Vec::new(),
            joined: Revision::new(),
            current: Tracked::new(None),
            pending_account: Tracked::new(None),
            error_message: Tracked::new(None),
            connection_error: Tracked::new(HashMap::new()),
            notices: Notices::default(),
            focused: true,
            signing: Signing::new(),
            sidebar: Sidebar::new(),
            conversation: Conversation::new(),
            calls: Calls::new(),
            screens: Screens::new(),
            profiles: Profiles::new(),
            member_menu: MemberMenu::new(),
            friends: Friends::new(),
            switcher: Switcher::new(),
            space_sheet: SpaceSheet::new(),
            settings: Settings::new(),
            menus: Menus::new(),
            manage: Manage::new(),
            space_admin: SpaceAdmin::new(),
            channel_admin: ChannelAdmin::new(),
        }
    }

    pub(crate) fn links(&self) -> &[LinkedAccount] {
        self.joined.read();
        &self.links
    }

    pub(crate) fn member_menu(&self) -> &MemberMenu {
        &self.member_menu
    }

    pub(crate) fn open_member_menu(
        &mut self,
        user: &UserId,
        space: Option<&SpaceId>,
        anchor: Anchor,
    ) {
        let Some(key) = self.current_key().map(str::to_owned) else {
            return;
        };
        self.close_profiles();
        self.member_menu.show(MemberTarget {
            key,
            user: *user,
            space: space.cloned(),
            anchor,
            over_profile: false,
        });
    }

    pub(crate) fn close_member_menu(&mut self) {
        self.member_menu.close();
    }

    pub(crate) fn friends(&self) -> &Friends {
        &self.friends
    }

    pub(crate) fn switcher(&self) -> &Switcher {
        &self.switcher
    }

    pub(crate) fn switcher_mut(&mut self) -> &mut Switcher {
        &mut self.switcher
    }

    pub(crate) fn toggle_switcher(&mut self) {
        if self.current().is_none() {
            return;
        }
        self.close_profiles();
        self.member_menu.close();
        self.sidebar.close_menu();
        self.space_sheet.close();
        self.switcher.toggle();
    }

    pub(crate) fn close_overlays(&mut self) {
        self.close_profiles();
        self.member_menu.close();
        self.sidebar.close_menu();
        self.switcher.close();
        self.space_sheet.close();
        self.menus.close();
    }

    pub(crate) fn space_admin(&self) -> &SpaceAdmin {
        &self.space_admin
    }

    pub(crate) fn space_admin_mut(&mut self) -> &mut SpaceAdmin {
        &mut self.space_admin
    }

    pub(crate) fn channel_admin(&self) -> &ChannelAdmin {
        &self.channel_admin
    }

    pub(crate) fn channel_admin_mut(&mut self) -> &mut ChannelAdmin {
        &mut self.channel_admin
    }

    pub(crate) fn escape(&mut self) {
        if self.manage.mfa().is_some() {
            self.cancel_mfa();
            return;
        }
        if self.manage.prompt().is_some() {
            self.answer_prompt(false);
            return;
        }
        if self.manage.alert().is_some() {
            self.manage.dismiss_alert();
            return;
        }
        if self.manage.dialog().is_some() {
            self.manage.close();
            return;
        }
        if self.menus.shown().is_some() {
            self.menus.close();
            return;
        }
        if self.member_menu.shown().is_some() {
            self.member_menu.close();
            return;
        }
        if self.channel_admin.is_open() {
            self.close_channel_admin();
            return;
        }
        if self.space_admin.is_open() {
            self.close_space_admin();
            return;
        }
        if self.settings.is_open() {
            self.close_settings();
            return;
        }
        if self.space_sheet.is_open() {
            self.space_sheet.close();
            return;
        }
        self.close_overlays();
    }

    pub(crate) fn stage(&self) -> Stage {
        if self.signing.is_open() {
            return Stage::SignIn;
        }
        if self.current().is_some() {
            return Stage::Desk;
        }
        match self.pending_account.is_some() {
            true => Stage::Connecting,
            false => Stage::SignIn,
        }
    }

    pub(crate) fn accounts(&self) -> &Accounts<SavedAccount> {
        &self.accounts
    }

    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub(crate) fn connection_error(&self, key: &str) -> Option<&str> {
        self.connection_error.get(key).map(String::as_str)
    }

    pub(crate) fn pending_account(&self) -> Option<&SavedAccount> {
        self.pending_account
            .as_deref()
            .and_then(|key| self.accounts.find(key))
    }

    pub(crate) fn current_key(&self) -> Option<&str> {
        self.current.as_deref()
    }

    pub(crate) fn current(&self) -> Option<&LinkedAccount> {
        let key = self.current.as_deref()?;
        self.linked(key)
    }

    pub(crate) fn current_mut(&mut self) -> Option<&mut LinkedAccount> {
        let key = (*self.current).clone()?;
        self.linked_mut(&key)
    }

    pub(crate) fn linked(&self, key: &str) -> Option<&LinkedAccount> {
        self.joined.read();
        self.links.iter().find(|linked| linked.key() == key)
    }

    pub(crate) fn linked_mut(&mut self, key: &str) -> Option<&mut LinkedAccount> {
        self.links.iter_mut().find(|linked| linked.key() == key)
    }

    pub(crate) fn is_linked(&self, key: &str) -> bool {
        self.linked(key).is_some()
    }

    pub(crate) fn has_links(&self) -> bool {
        self.joined.read();
        !self.links.is_empty()
    }

    pub(crate) fn has_desk(&self) -> bool {
        self.has_links() || self.pending_account.is_some()
    }

    pub(crate) fn signing(&self) -> &Signing {
        &self.signing
    }

    pub(crate) fn sidebar(&self) -> &Sidebar {
        &self.sidebar
    }

    pub(crate) fn conversation(&self) -> &Conversation {
        &self.conversation
    }

    pub(crate) fn conversation_mut(&mut self) -> &mut Conversation {
        &mut self.conversation
    }

    pub(crate) fn dismiss_trouble(&mut self) {
        self.error_message.set(None);
    }
}

impl std::fmt::Debug for Chat {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Chat")
            .field("current", &*self.current)
            .field("links", &self.links)
            .finish_non_exhaustive()
    }
}
