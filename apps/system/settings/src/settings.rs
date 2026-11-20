use rikka_app::prelude::*;
use rikka_llm_settings::{LlmSettings, llm_settings};
use rikka_nix::{Progress, Stage};
use rikka_session::settings_app::{Link, Spot};
use rikka_sound::NodeKind;
use rikka_widgets::column;

use crate::appearance::{self, Appearance};
use crate::audio::{self, Audio};
use crate::bluetooth::{self, Bluetooth};
use crate::capture::{self, Capture};
use crate::color_picker::{self, ColorPicker};
use crate::date_time::{self, DateTimePage};
use crate::defaults::{self, Defaults};
use crate::displays::{self, Displays};
use crate::hardware::Hardware;
use crate::keyboard::{self, Keyboard};
use crate::keyring::{self, Keyring};
use crate::language::{self, Language};
use crate::lock::{self, Lock};
use crate::network::{self, Network};
use crate::notifications::{self, Notifications};
use crate::page::Page;
use crate::pending::{Pending, panel};
use crate::pointer::{self, Pointer};
use crate::power::{self, PowerPage};
use crate::remote::{self, Remote};
use crate::replay::{self, Replay};
use crate::section::{self, Entry, Needs, Section};
use crate::session::Session;
use crate::system::{self, System};
use crate::users::{self, Users};
use crate::windows::{self, Windows};

#[derive(State)]
pub(crate) struct Settings {
    section: Section,
    behind: Vec<Section>,
    ahead: Vec<Section>,
    hardware: Hardware,
    users: Users,
    #[state]
    appearance: Appearance,
    windows: Windows,
    notifications: Notifications,
    lock: Lock,
    keyring: Keyring,
    capture: Capture,
    replay: Replay,
    color_picker: ColorPicker,
    displays: Displays,
    pointer: Pointer,
    #[state]
    keyboard: Keyboard,
    #[state]
    audio: Audio,
    power: PowerPage,
    #[state]
    network: Network,
    bluetooth: Bluetooth,
    remote: Remote,
    language: Language,
    #[state]
    date_time: DateTimePage,
    defaults: Defaults,
    llm: LlmSettings,
    system: System,
    pending: Pending,
    session: Session,
}

impl Settings {
    pub(crate) fn new(sender: Sender<Settings>) -> Self {
        Self {
            section: Link::from_arguments(std::env::args_os())
                .map(Link::section)
                .unwrap_or_default(),
            behind: Vec::new(),
            ahead: Vec::new(),
            hardware: Hardware::probe(),
            users: Users::new(),
            appearance: Appearance::new(
                rikka_appearance::Appearance::current()
                    .registry()
                    .ok()
                    .as_ref(),
            ),
            windows: Windows::new(),
            notifications: Notifications::new(),
            lock: Lock::new(),
            keyring: Keyring::new(),
            capture: Capture::new(),
            replay: Replay::new(),
            color_picker: ColorPicker::new(),
            displays: Displays::new(),
            pointer: Pointer::new(),
            keyboard: Keyboard::new(),
            audio: Audio::new(),
            power: PowerPage::new(),
            network: Network::new(),
            bluetooth: Bluetooth::new(),
            remote: Remote::new(),
            language: Language::new(),
            date_time: DateTimePage::new(),
            defaults: Defaults::new(),
            llm: LlmSettings::load(),
            system: System::new(),
            pending: Pending::new(move |progress| {
                let _ = sender.send(move |settings: &mut Settings| {
                    settings.rebuild_progressed(progress);
                });
            }),
            session: Session::connect(),
        }
    }

    pub(crate) fn start(&mut self) {
        self.open(self.section);
        if let Some(spot) = Link::from_arguments(std::env::args_os()).and_then(Link::spot) {
            self.reveal(spot);
        }
    }

    pub(crate) fn open(&mut self, section: Section) {
        self.hardware = Hardware::probe();
        let section = section::available(self.hardware)
            .find(|offered| *offered == section)
            .or_else(|| section::available(self.hardware).next())
            .unwrap_or_default();
        let (leaving, ..) = self.page(self.section);
        leaving.closed();
        self.section = section;
        let (page, pending, session) = self.page(section);
        page.opened(pending, session);
    }

    pub(crate) fn visit(&mut self, section: Section) {
        if section == self.section {
            return;
        }
        self.behind.push(self.section);
        self.ahead.clear();
        self.open(section);
    }

    pub(crate) fn opened_with(&mut self, open: LaunchRequest) {
        let Some(link) = Link::from_arguments(open.arguments()) else {
            skip_view();
            return;
        };
        self.visit(link.section());
        if let Some(spot) = link.spot() {
            self.reveal(spot);
        }
    }

    fn reveal(&mut self, spot: Spot) {
        if self.section != spot.section() {
            return;
        }
        match spot {
            Spot::Wallpaper => self.appearance.reveal_wallpapers(),
            Spot::SoundOutput => self.audio.reveal(NodeKind::Sink),
            Spot::SoundInput => self.audio.reveal(NodeKind::Source),
            Spot::InputSources => self.keyboard.reveal_sources(),
        }
    }

    pub(crate) fn back(&mut self) {
        if self.section == Section::Network && self.network.go_back() {
            return;
        }
        let Some(previous) = self.behind.pop() else {
            return;
        };
        self.ahead.push(self.section);
        self.open(previous);
    }

    pub(crate) fn forward(&mut self) {
        let Some(next) = self.ahead.pop() else {
            return;
        };
        self.behind.push(self.section);
        self.open(next);
    }

    pub(crate) fn choose_offer(&mut self, index: usize) {
        self.keyboard.choose(index, &mut self.pending);
    }

    pub(crate) fn write_remote(&mut self, key: &'static str, value: impl serde::Serialize) {
        self.remote.write(&mut self.pending, key, value);
    }

    pub(crate) fn commit_time_servers(&mut self) {
        self.date_time.commit_servers(&mut self.pending);
    }

    pub(crate) fn choose_zone(&mut self, index: usize) {
        self.date_time.choose_zone(&mut self.pending, index);
    }

    pub(crate) fn rename_user(&mut self) {
        self.users.commit_name(&mut self.pending);
    }

    fn page(&mut self, section: Section) -> (&mut dyn Page, &Pending, &Session) {
        let page: &mut dyn Page = match section {
            Section::Users => &mut self.users,
            Section::Appearance => &mut self.appearance,
            Section::Windows => &mut self.windows,
            Section::Notifications => &mut self.notifications,
            Section::Lock => &mut self.lock,
            Section::Keyring => &mut self.keyring,
            Section::Capture => &mut self.capture,
            Section::Replay => &mut self.replay,
            Section::ColorPicker => &mut self.color_picker,
            Section::Displays => &mut self.displays,
            Section::Pointer => &mut self.pointer,
            Section::Keyboard => &mut self.keyboard,
            Section::Audio => &mut self.audio,
            Section::Power => &mut self.power,
            Section::Network => &mut self.network,
            Section::Bluetooth => &mut self.bluetooth,
            Section::Remote => &mut self.remote,
            Section::Language => &mut self.language,
            Section::DateTime => &mut self.date_time,
            Section::Defaults => &mut self.defaults,
            Section::Llm => &mut self.llm,
            Section::System => &mut self.system,
        };
        (page, &self.pending, &self.session)
    }

    fn rebuild_progressed(&mut self, progress: Progress) {
        if self.pending.progress(progress) {
            self.rebuild_finished();
        }
    }

    fn apply(&mut self) {
        if self.pending.apply() {
            self.rebuild_finished();
        }
    }

    fn discard(&mut self) {
        self.pending.discard();
        self.rebuild_finished();
    }

    fn rebuild_finished(&mut self) {
        self.system.rebuild_finished();
        self.keyboard.rebuild_finished();
        self.users.rebuild_finished(&self.pending);
    }

    fn entries(&self) -> Vec<Entry> {
        section::entries(self.hardware)
    }

    fn at(&self, entries: &[Entry]) -> Option<usize> {
        entries
            .iter()
            .position(|entry| *entry == Entry::Item(self.section))
    }
}
pub(crate) fn view(settings: &Settings) -> impl View {
    let entries = settings.entries();
    split_view(nav(settings, &entries), body(settings))
        .heading(section::title(settings.section))
        .shows(settings.at(&entries).unwrap_or(0))
}

fn nav(settings: &Settings, entries: &[Entry]) -> impl View {
    let visible = entries.to_vec();
    let at = settings.at(entries).unwrap_or(0);
    let waiting = settings.pending.changed();
    lazy((visible.clone(), at, waiting), move || {
        let visible = visible.clone();
        nav_list(at)
            .items(visible.iter().map(|entry| {
                let item = entry.nav_item();
                match entry {
                    Entry::Item(section) if section::drafts(*section) && waiting => {
                        item.tint(palette().accent)
                    }
                    _ => item,
                }
            }))
            .on_select(move |settings: &mut Settings, index: usize| {
                let section = match visible.get(index) {
                    Some(Entry::Item(section)) => *section,
                    _ => Section::default(),
                };
                settings.visit(section);
            })
            .fill()
    })
}

fn body(settings: &Settings) -> impl View {
    let pending = &settings.pending;
    column! {
        waiting(settings);
        if !(section::needs(settings.section) == Needs::Nixos && pending.unavailable().is_some()) {
            match settings.section {
                Section::Users => users::view(&settings.users, pending),
                Section::Appearance => appearance::view(&settings.appearance),
                Section::Windows => windows::view(&settings.windows),
                Section::Notifications => notifications::view(&settings.notifications),
                Section::Lock => lock::view(&settings.lock),
                Section::Keyring => keyring::view(&settings.keyring),
                Section::Capture => capture::view(&settings.capture),
                Section::Replay => replay::view(&settings.replay),
                Section::ColorPicker => color_picker::view(&settings.color_picker),
                Section::Displays => displays::view(&settings.displays),
                Section::Pointer => pointer::view(&settings.pointer),
                Section::Keyboard => keyboard::view(&settings.keyboard, pending),
                Section::Audio => component(|settings: &Settings| &settings.audio, audio::view),
                Section::Power => power::view(&settings.power, pending),
                Section::Network => network::view(&settings.network),
                Section::Bluetooth => bluetooth::view(&settings.bluetooth),
                Section::Remote => remote::view(&settings.remote, pending),
                Section::Language => language::view(&settings.language, pending),
                Section::DateTime => date_time::view(&settings.date_time, pending),
                Section::Defaults => defaults::view(&settings.defaults),
                Section::Llm => llm_settings(&settings.llm),
                Section::System => system::view(&settings.system, pending),
            }
        }
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn waiting(settings: &Settings) -> Option<Element> {
    if !section::drafts(settings.section) && settings.pending.stage() == Stage::Idle {
        return None;
    }
    panel(
        &settings.pending,
        Handler::new(|settings: &mut Settings| settings.apply()),
        Handler::new(|settings: &mut Settings| settings.discard()),
    )
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("section", &self.section)
            .finish_non_exhaustive()
    }
}
