mod blending;
mod local;
mod output;
mod playlists;
mod storage;

use std::sync::Arc;
use std::time::Duration;

use music_blend::Store;
use music_core::{
    Account, AudioCache, Catalog, Connection, Cookie, Prompt, Providers, Quality, SavedAccount,
    Service, SignInStep, Track, WebSignIn,
};
use music_preferences::Preferences;
use rikka_account_book::{Accounts, PendingAccount};
use rikka_app::prelude::*;
use rikka_web::{CloseReason, Event};

use crate::artwork;
use crate::blends::Blends;
use crate::dialogs::Dialogs;
use crate::library::Library;
use crate::local::Arrange;
use crate::menus::Menus;
use crate::nav::{Nav, Place};
use crate::player::Player;
use crate::portal::Portal;
use crate::settings::Settings;
use crate::sound::Sound;

pub(crate) use playlists::Start;

const SEARCH_PAUSE: Duration = Duration::from_millis(320);
const SIGN_IN: &str = "music-sign-in";
const RESUME: &str = "music-resume";
const PORTAL: &str = "music-portal";
const PORTAL_COOKIES: &str = "music-portal-cookies";
const BLEND_FAILURES: &str = "music-blend-failures";
const BLEND_OUTSIDE: &str = "music-blend-outside";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Connecting,
    Choosing,
    Browser { service: Service, url: String },
    Page(Service),
    Ready,
}

#[derive(State)]
pub(crate) struct Music {
    #[untracked]
    sender: Sender<Music>,
    #[untracked]
    providers: Providers,
    accounts: Tracked<Accounts<SavedAccount>>,
    stage: Tracked<Stage>,
    error_message: Tracked<Option<String>>,
    #[untracked]
    pending_key: Option<String>,
    #[untracked]
    portal: Option<Portal>,
    preferences: Tracked<Preferences>,
    #[untracked]
    audio: AudioCache,
    #[untracked]
    local: Option<music_library::Library>,
    scanning: Tracked<music_library::Progress>,
    indexed: Tracked<u64>,
    listened: Tracked<u64>,
    stored: Tracked<u64>,
    settings_open: Tracked<bool>,
    queue_open: Tracked<bool>,
    signal_open: Tracked<bool>,
    #[untracked]
    field: TextDocument,
    seeking: Tracked<bool>,
    #[untracked]
    searched: u64,
    search_query: Tracked<String>,
    #[state]
    nav: Nav,
    #[state]
    player: Player,
    #[state]
    sound: Sound,
    #[state]
    library: Library,
    #[state]
    arrange: Arrange,
    #[state]
    blends: Blends,
    #[state]
    settings: Settings,
    #[state]
    menus: Menus,
    #[state]
    dialogs: Dialogs,
}

impl Music {
    pub(crate) fn new(
        sender: Sender<Music>,
        providers: Providers,
        accounts: Accounts<SavedAccount>,
        preferences: Preferences,
        audio: AudioCache,
        store: Store,
        local: Option<music_library::Library>,
    ) -> Music {
        let order = ordered(&providers, preferences.order());
        let blends = Blends::new(store);
        let output = providers.output().clone();
        let sound = Sound::new(
            output.clone(),
            preferences.output(),
            preferences.replay_gain(),
        );
        Music {
            sender,
            player: Player::new(order, blends.known(), output),
            sound,
            providers,
            accounts: Tracked::new(accounts),
            stage: Tracked::new(Stage::Connecting),
            error_message: Tracked::new(None),
            pending_key: None,
            portal: None,
            preferences: Tracked::new(preferences),
            audio,
            local,
            scanning: Tracked::default(),
            indexed: Tracked::new(0),
            listened: Tracked::new(0),
            stored: Tracked::new(0),
            settings_open: Tracked::new(false),
            queue_open: Tracked::new(false),
            signal_open: Tracked::new(false),
            field: TextDocument::new(""),
            seeking: Tracked::new(false),
            searched: 0,
            search_query: Tracked::new(String::new()),
            nav: Nav::new(),
            library: Library::new(),
            arrange: Arrange::default(),
            blends,
            settings: Settings::new(),
            menus: Menus::new(),
            dialogs: Dialogs::new(),
        }
    }

    pub(crate) fn start(&mut self) {
        artwork::set_quality(self.preferences.image());
        crate::mpris::publish();
        self.sound.listen();
        stream_keyed(
            BLEND_FAILURES,
            self.blends.failures(),
            |library: &mut Library, error: music_core::Error| library.failed(&error),
        );
        stream_keyed(
            BLEND_OUTSIDE,
            self.blends.outside(),
            |music: &mut Music, outside: music_blend::Outside| music.blend_outside(outside),
        );
        self.watch_local();
        self.pending_key = self
            .accounts
            .current()
            .map(|saved| saved.key.clone())
            .or_else(|| self.local_key());
        if self.pending_key.is_none() {
            self.stage.set(Stage::Choosing);
        }
        self.link_local();
        for saved in self.accounts.all().to_vec() {
            self.resume(&saved);
        }
    }

    pub(crate) fn providers(&self) -> &Providers {
        &self.providers
    }

    pub(crate) fn accounts(&self) -> &Accounts<SavedAccount> {
        &self.accounts
    }

    pub(crate) fn stage(&self) -> &Stage {
        &self.stage
    }

    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub(crate) fn connection(&self) -> Option<&Connection> {
        self.player.linked(self.player.browsed()?)
    }

    pub(crate) fn account_key(&self) -> &str {
        self.player.browsed().unwrap_or_default()
    }

    pub(crate) fn connections(&self) -> Vec<Connection> {
        self.player.connections()
    }

    pub(crate) fn has_links(&self) -> bool {
        self.player.first_linked().is_some()
    }

    pub(crate) fn is_linked(&self, key: &str) -> bool {
        self.player.linked(key).is_some()
    }

    pub(crate) fn browses(&self, track: &Track) -> bool {
        self.connection()
            .is_some_and(|connection| connection.service.slug == track.service.slug)
    }

    pub(crate) fn order(&self) -> Vec<String> {
        ordered(&self.providers, self.preferences.order())
    }

    pub(crate) fn catalog(&self) -> Option<Arc<dyn Catalog>> {
        self.connection()
            .map(|connection| Arc::clone(&connection.catalog))
    }

    pub(crate) fn portal(&self) -> Option<&Portal> {
        self.portal.as_ref()
    }

    pub(crate) fn preferences(&self) -> &Preferences {
        &self.preferences
    }

    pub(crate) fn settings_open(&self) -> bool {
        *self.settings_open
    }

    pub(crate) fn queue_open(&self) -> bool {
        *self.queue_open
    }

    pub(crate) fn field(&self) -> &TextDocument {
        &self.field
    }

    pub(crate) fn search_query(&self) -> &str {
        &self.search_query
    }

    pub(crate) fn nav(&self) -> &Nav {
        &self.nav
    }

    pub(crate) fn player(&self) -> &Player {
        &self.player
    }

    pub(crate) fn library(&self) -> &Library {
        &self.library
    }

    pub(crate) fn blends(&self) -> &Blends {
        &self.blends
    }

    pub(crate) fn settings(&self) -> &Settings {
        &self.settings
    }

    pub(crate) fn menus(&self) -> &Menus {
        &self.menus
    }

    pub(crate) fn dialogs(&self) -> &Dialogs {
        &self.dialogs
    }

    pub(crate) fn arrange(&self) -> &Arrange {
        &self.arrange
    }

    pub(crate) fn open_settings(&mut self, open: bool) {
        self.error_message.set(None);
        self.leave_desk(open);
        if open {
            self.stored.set(*self.stored + 1);
            self.sound.list_devices();
            self.sound.watch_graph();
        }
        self.settings_open.set(open);
    }

    pub(crate) fn set_audio_quality(&mut self, quality: Quality) {
        if self.preferences.audio() == quality {
            return;
        }
        self.preferences.set_audio(quality);
        self.player.set_quality(quality);
    }

    pub(crate) fn set_image_quality(&mut self, quality: Quality) {
        if self.preferences.image() == quality {
            return;
        }
        self.preferences.set_image(quality);
        artwork::set_quality(quality);
    }

    pub(crate) fn move_service(&mut self, slug: &str, earlier: bool) {
        let mut order = self.order();
        let Some(at) = order.iter().position(|preferred| preferred == slug) else {
            return;
        };
        let to = match earlier {
            true => at.checked_sub(1),
            false => Some(at + 1).filter(|to| *to < order.len()),
        };
        let Some(to) = to else {
            return;
        };
        order.swap(at, to);
        self.preferences.set_order(&order);
        self.player.set_order(order);
    }

    pub(crate) fn switch(&mut self, key: &str) {
        if self.account_key() == key {
            return;
        }
        if self.is_linked(key) {
            self.error_message.set(None);
            self.choose(key);
            return;
        }
        if let Some(saved) = self.accounts.find(key).cloned() {
            self.error_message.set(None);
            self.pending_key = Some(saved.key.clone());
            self.stage.set(Stage::Connecting);
            self.resume(&saved);
        }
    }

    pub(crate) fn forget(&mut self, key: &str) {
        cancel(resume_key(key));
        self.unlink(key);
        if self.pending_key.as_deref() == Some(key) {
            self.pending_key = None;
            self.restore_stage();
        }
        if let Err(error) = self.accounts.forget(key) {
            self.error_message.set(Some(error.to_string()));
        }
    }

    pub(crate) fn sign_in(&mut self, slug: &'static str) {
        let Some(provider) = self.providers.find(slug).cloned() else {
            return;
        };
        let pending = match self.accounts.reserve() {
            Ok(pending) => pending,
            Err(error) => {
                self.error_message.set(Some(error.to_string()));
                return;
            }
        };
        let service = provider.service();
        self.error_message.set(None);
        self.stage.set(Stage::Connecting);
        let sender = self.sender.clone();
        let prompt: Prompt = Arc::new(move |step: SignInStep| match step {
            SignInStep::Browse(url) => {
                if let Err(error) = rikka_open::open(&url) {
                    tracing::warn!("failed to open sign-in page: {error}");
                }
                let _ = sender.send(move |music: &mut Music| music.browse(service, url));
            }
            SignInStep::Web(request) => {
                let _ = sender.send(move |music: &mut Music| music.open_portal(service, request));
            }
        });
        let folder = pending.folder().to_owned();
        spawn_keyed(
            SIGN_IN,
            async move {
                let signed = provider
                    .sign_in(&folder, prompt)
                    .await
                    .map_err(|error| error.to_string());
                (pending, signed)
            },
            |music: &mut Music, (pending, signed): (PendingAccount, Result<Connection, String>)| {
                music.signed(pending, signed);
            },
        );
    }

    pub(crate) fn cancel_connecting(&mut self) {
        cancel(SIGN_IN);
        if let Some(account_key) = self.pending_key.take()
            && !self.is_linked(&account_key)
        {
            cancel(resume_key(&account_key));
        }
        self.close_portal();
        self.restore_stage();
    }

    pub(crate) fn toggle_queue(&mut self) {
        let open = !*self.queue_open;
        self.queue_open.set(open);
    }

    fn leave_desk(&mut self, leaving: bool) {
        if leaving {
            self.menus.close();
            self.dialogs.close();
            self.close_signal();
        }
    }

    pub(crate) fn typed(&mut self) {
        let revision = self.field.revision();
        if revision == self.searched {
            skip_view();
            return;
        }
        self.searched = revision;
        after_keyed("music-search", SEARCH_PAUSE, |music: &mut Music| {
            music.search_now();
        });
    }

    pub(crate) fn search_now(&mut self) {
        let query = self.field.text().trim().to_owned();
        self.search_query.set(query);
        if self.nav.current() != &Place::Search {
            self.nav.root(Place::Search);
        }
    }

    pub(crate) fn go(&mut self, place: Place) {
        self.nav.go(place);
        self.arrived();
    }

    pub(crate) fn find(&mut self) {
        if *self.settings_open {
            self.open_settings(false);
        }
        self.field.select_all();
        self.seeking.set(true);
    }

    pub(crate) fn seek_finished(&mut self) {
        self.seeking.set(false);
    }

    pub(crate) fn seeking(&self) -> bool {
        *self.seeking
    }

    pub(crate) fn back(&mut self) {
        if *self.settings_open {
            self.open_settings(false);
            return;
        }
        self.nav.back();
        self.arrived();
    }

    pub(crate) fn forward(&mut self) {
        self.nav.forward();
        self.arrived();
    }

    pub(crate) fn root(&mut self, place: Place) {
        self.nav.root(place);
        self.arrived();
    }

    fn arrived(&mut self) {
        let nav = &self.nav;
        self.library
            .retain_pages(|id| nav.holds(&Place::Playlist(id.clone())));
        if let Place::Playlist(id) = self.nav.current().clone() {
            self.library.visit(&id);
        }
    }

    pub(crate) fn listen(&mut self, track: Track) {
        self.player.play(&[track], 0);
    }

    pub(crate) fn connected(&mut self, key: String, connection: Connection) {
        self.error_message.set(None);
        self.link(&key, connection);
        self.choose(&key);
    }

    fn link(&mut self, key: &str, connection: Connection) {
        self.player.link(key, connection, self.preferences.audio());
        self.discover_soon();
    }

    fn unlink(&mut self, key: &str) {
        if !self.is_linked(key) {
            return;
        }
        let browsed = self.account_key() == key;
        if browsed {
            self.release();
        }
        self.player.unlink(key);
        if !browsed {
            return;
        }
        match self.player.first_linked() {
            Some(next) => self.choose(&next),
            None => self.stage.set(Stage::Choosing),
        }
    }

    fn choose(&mut self, key: &str) {
        let Some(connection) = self.player.linked(key).cloned() else {
            return;
        };
        self.release();
        self.pending_key = None;
        self.player.browse(Some(key.to_owned()));
        self.library.attach(Arc::clone(&connection.catalog));
        self.remember(key, &connection.account);
        self.stage.set(Stage::Ready);
    }

    fn resume(&mut self, saved: &SavedAccount) {
        let Some(provider) = self.providers.find(&saved.provider).cloned() else {
            self.fall_back(
                tr!("music.accounts.unknown", name = saved.provider.as_str()).to_string(),
            );
            return;
        };
        if self.pending_key.as_deref() == Some(saved.key.as_str()) {
            self.error_message.set(None);
            self.stage.set(Stage::Connecting);
        }
        let folder = self.accounts.folder(&saved.key);
        let saved = saved.clone();
        spawn_keyed(
            resume_key(&saved.key),
            async move {
                provider
                    .resume(&folder)
                    .await
                    .map_err(|error| error.to_string())
            },
            move |music: &mut Music, resumed: Result<Option<Connection>, String>| {
                music.resumed(&saved, resumed);
            },
        );
    }

    pub(crate) fn resumed(
        &mut self,
        saved: &SavedAccount,
        resumed: Result<Option<Connection>, String>,
    ) {
        let is_pending = self.pending_key.as_deref() == Some(saved.key.as_str());
        match resumed {
            Ok(Some(connection)) => {
                if let Err(error) = self.accounts.update(&saved.key, |known| {
                    known.account = connection.account.clone()
                }) {
                    tracing::warn!("failed to save account: {error}");
                }
                self.link(&saved.key, connection);
                if is_pending {
                    self.error_message.set(None);
                    self.choose(&saved.key);
                }
            }
            Ok(None) => {
                if let Err(error) = self.accounts.forget(&saved.key) {
                    tracing::warn!("failed to forget signed-out account: {error}");
                }
                let message =
                    tr!("music.accounts.expired", name = saved.account.name.as_str()).to_string();
                match is_pending {
                    true => {
                        self.pending_key = None;
                        self.fall_back(message);
                    }
                    false => self.library.show_note(message),
                }
            }
            Err(reason) if is_pending => {
                self.pending_key = None;
                self.fall_back(reason);
            }
            Err(reason) => {
                tracing::warn!("{} unreachable: {reason}", saved.account.name);
            }
        }
    }

    fn remember(&mut self, key: &str, account: &Account) {
        let current = self
            .accounts
            .current()
            .map(|saved| (saved.key.as_str(), &saved.account));
        if current == Some((key, account)) {
            return;
        }
        let saved = self.accounts.choose(key).and_then(|()| {
            self.accounts
                .update(key, |known| known.account = account.clone())
        });
        if let Err(error) = saved {
            tracing::warn!("failed to save current account: {error}");
        }
    }

    fn signed(&mut self, pending: PendingAccount, signed: Result<Connection, String>) {
        self.close_portal();
        let committed = signed.and_then(|connection| {
            let provider = connection.service.slug.to_owned();
            let account = connection.account.clone();
            self.accounts
                .commit(pending, |key| SavedAccount {
                    key,
                    provider,
                    account,
                })
                .map(|committed| (committed.saved.key, connection))
                .map_err(|error| error.to_string())
        });
        match committed {
            Ok((key, connection)) => self.connected(key, connection),
            Err(reason) => self.fall_back(reason),
        }
    }

    fn fall_back(&mut self, error_message: String) {
        self.restore_stage();
        self.error_message.set(Some(error_message));
    }

    fn restore_stage(&mut self) {
        if self.connection().is_some() {
            self.stage.set(Stage::Ready);
        } else if !self.browse_local() {
            self.stage.set(Stage::Choosing);
        }
    }

    fn release(&mut self) {
        if self.player.browsed().is_none() {
            return;
        }
        self.library.detach();
        self.menus.close();
        self.dialogs.close();
        self.nav = Nav::new();
        self.settings_open.set(false);
        self.player.browse(None);
    }

    fn browse(&mut self, service: Service, url: String) {
        if matches!(*self.stage, Stage::Connecting | Stage::Browser { .. }) {
            self.stage.set(Stage::Browser { service, url });
        }
    }

    fn open_portal(&mut self, service: Service, request: WebSignIn) {
        if *self.stage != Stage::Connecting {
            return;
        }
        let portal = match Portal::open(request) {
            Ok(portal) => portal,
            Err(reason) => {
                cancel(SIGN_IN);
                self.fall_back(reason);
                return;
            }
        };
        if let Some(events) = portal.session().events() {
            stream_keyed(PORTAL, events, |music: &mut Music, event: Event| {
                music.portal_heard(event);
            });
        }
        self.portal = Some(portal);
        self.stage.set(Stage::Page(service));
    }

    fn portal_heard(&mut self, event: Event) {
        skip_view();
        let Some(portal) = &self.portal else {
            return;
        };
        match event {
            Event::NewSession(opening) => portal.follow(opening),
            Event::Changed | Event::Finished { .. } if portal.is_on_landing_host() => {
                spawn_keyed(
                    PORTAL_COOKIES,
                    portal.cookies(),
                    |music: &mut Music, cookies: Result<Vec<Cookie>, String>| {
                        music.portal_cookies(cookies);
                    },
                );
            }
            Event::Closed(CloseReason::EngineExited) => {
                cancel(SIGN_IN);
                self.close_portal();
                self.fall_back(tr!("music.signing.page-closed").to_string());
            }
            _ => {}
        }
    }

    fn portal_cookies(&mut self, result: Result<Vec<Cookie>, String>) {
        let cookies = match result {
            Ok(cookies) => cookies,
            Err(reason) => {
                tracing::warn!("failed to read sign-in cookies: {reason}");
                skip_view();
                return;
            }
        };
        let Some(portal) = self
            .portal
            .take_if(|portal| portal.has_proof_cookie(&cookies))
        else {
            skip_view();
            return;
        };
        cancel(PORTAL);
        portal.finish(cookies);
        self.stage.set(Stage::Connecting);
    }

    fn close_portal(&mut self) {
        cancel(PORTAL);
        cancel(PORTAL_COOKIES);
        self.portal = None;
    }
}

fn resume_key(key: &str) -> (&'static str, String) {
    (RESUME, key.to_owned())
}

fn ordered(providers: &Providers, preferred: Vec<String>) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    for slug in preferred {
        if providers.find(&slug).is_some() && !order.contains(&slug) {
            order.push(slug);
        }
    }
    for service in providers.services() {
        if !order.iter().any(|preferred| preferred == service.slug) {
            order.push(service.slug.to_owned());
        }
    }
    order
}

impl std::fmt::Debug for Music {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Music")
            .field("stage", &*self.stage)
            .field("nav", &self.nav)
            .finish_non_exhaustive()
    }
}
