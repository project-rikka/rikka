use rikka_compositor_planes::Tearing;
use rikka_ime::Keymap;
use rikka_input::ActionMap;
use rikka_seam::{Delivery, EventQueue, Seam, SeamClient};
use rikka_session::profile::Profile;
use rikka_session::shortcuts;
use rikka_settings::seam::{CAPABILITY, Changed, Settings, SettingsService, protocol};
use rikka_settings::session::lock;
use rikka_settings::{Layers, Schema};
use rikka_theme::Palette;

use crate::input::{KeyAction, PointerRules};
use crate::outputs::OutputConfig;
use crate::policy::{CaptionLook, Glass, WindowRules};
use crate::session::SessionKind;
use crate::state::Compositor;

use super::appearance::GlassStyle;
use super::displays::Display;
use super::service::HostedSettings;
use super::{animation, appearance, displays, ime, keybindings, network, pointer, windows};

pub(crate) struct SessionSettings {
    client: SeamClient,
    profile: Profile,
    announcements: EventQueue,
    layers: Layers,
    input: Settings,
    animation: Settings,
    windows: Settings,
    pointer: Settings,
    displays: Settings,
    network: Settings,
    ime: Settings,
    appearance: Settings,
    glazier: GlassStyle,
}

struct Domains {
    service: SettingsService,
    input: Settings,
    animation: Settings,
    windows: Settings,
    pointer: Settings,
    displays: Settings,
    network: Settings,
    ime: Settings,
}

impl SessionSettings {
    pub(crate) fn start(
        seam: &mut Seam<Compositor>,
        profile: Profile,
        standing: &SessionKind,
    ) -> Option<Self> {
        let layers = standing
            .settings_layers()
            .inspect_err(|error| tracing::error!("cannot open session settings: {error}"))
            .ok()?;
        let domains = domains(&layers)
            .inspect_err(|error| tracing::error!("cannot open session settings: {error}"))
            .ok()?;
        let appearance = Settings::local_with_layers(appearance::schema(), layers.clone())
            .inspect_err(|error| tracing::error!("cannot read appearance settings: {error}"))
            .ok()?;

        seam.register(HostedSettings(domains.service))
            .inspect_err(|error| tracing::error!("cannot register `{CAPABILITY}`: {error}"))
            .ok()?;
        let client = seam.client();
        let announcements = client.poll_events(Some(CAPABILITY));

        let settings = Self {
            client,
            profile,
            announcements,
            layers,
            input: domains.input,
            animation: domains.animation,
            windows: domains.windows,
            pointer: domains.pointer,
            displays: domains.displays,
            network: domains.network,
            ime: domains.ime,
            appearance,
            glazier: GlassStyle::default(),
        };
        if profile.is_full() {
            settings.announce_declaration(&displays::schema(&[]));
        }
        Some(settings)
    }

    fn announce_declaration(&self, schema: &Schema) {
        self.client.notify(protocol::declare(schema));
    }

    pub(crate) fn action_map(&self) -> ActionMap<KeyAction> {
        keybindings::action_map(&self.input, self.profile)
    }

    pub(crate) fn keymap(&self) -> Keymap {
        ime::keymap(&self.ime)
    }

    pub(crate) fn animations_enabled(&self) -> bool {
        animation::enabled(&self.animation)
    }

    pub(crate) fn locks_at_login(&self) -> bool {
        Settings::local_with_layers(lock::schema(), self.layers.clone())
            .map(|settings| settings.read_or(lock::AT_LOGIN, lock::AT_LOGIN_DEFAULT))
            .unwrap_or_else(|error| {
                tracing::warn!("cannot read lock settings, starting locked: {error}");
                lock::AT_LOGIN_DEFAULT
            })
    }

    pub(crate) fn minimizes_windows(&self) -> bool {
        appearance::minimizes_windows(&self.appearance)
    }

    pub(crate) fn glass(&mut self) -> Glass {
        let Self {
            glazier,
            appearance,
            ..
        } = self;
        glazier.glass(appearance)
    }

    pub(crate) fn palette(&mut self) -> Palette {
        let Self {
            glazier,
            appearance,
            ..
        } = self;
        glazier.palette(appearance)
    }

    pub(crate) fn caption(&mut self) -> CaptionLook {
        let Self {
            glazier,
            appearance,
            ..
        } = self;
        glazier.caption(appearance)
    }

    pub(crate) fn window_rules(&self) -> WindowRules {
        windows::rules(&self.windows).for_profile(self.profile)
    }

    pub(crate) fn tearing(&self) -> Tearing {
        windows::tearing(&self.windows)
    }

    pub(crate) fn pointer_rules(&self) -> PointerRules {
        pointer::rules(&self.pointer)
    }

    pub(crate) fn proxy_environment(&self) -> Vec<(&'static str, Option<std::ffi::OsString>)> {
        network::environment(&self.network)
    }

    pub(crate) fn declares_displays(&self) -> bool {
        self.profile.is_full()
    }

    pub(crate) fn declare_displays(&mut self, displays: &[Display]) -> bool {
        let schema = displays::schema(displays);
        if *self.displays.schema() == schema {
            return false;
        }
        match Settings::local_with_layers(schema.clone(), self.layers.clone()) {
            Ok(mut settings) => {
                let mut values = settings.user_values().clone();
                values.extend(self.displays.user_values().clone());
                settings.set_user_values(values);
                self.displays = settings;
            }
            Err(error) => {
                tracing::warn!("cannot reload display settings: {error}");
                return false;
            }
        }
        self.announce_declaration(&schema);
        true
    }

    pub(crate) fn display_configs(&self, displays: &[Display]) -> Vec<OutputConfig> {
        displays
            .iter()
            .map(|display| display.config(&self.displays))
            .collect()
    }

    pub(crate) fn refresh(&mut self) -> Rebuild {
        let mut rebuild = Rebuild::default();
        while let Some(announcement) = self.announcements.try_next() {
            let event = match announcement {
                Delivery::Event(event) => event,
                Delivery::Lagged(_) => {
                    self.reread(&mut rebuild);
                    continue;
                }
            };
            let Ok(changed) = event.decode::<Changed>() else {
                continue;
            };
            let mirror = match changed.domain.as_str() {
                shortcuts::DOMAIN => rebuilt(&mut rebuild.keybindings, &mut self.input),
                animation::DOMAIN => rebuilt(&mut rebuild.policy, &mut self.animation),
                windows::DOMAIN => rebuilt(&mut rebuild.policy, &mut self.windows),
                pointer::DOMAIN => rebuilt(&mut rebuild.pointer, &mut self.pointer),
                displays::DOMAIN => rebuilt(&mut rebuild.displays, &mut self.displays),
                ime::DOMAIN => rebuilt(&mut rebuild.policy, &mut self.ime),
                appearance::DOMAIN => rebuilt(&mut rebuild.policy, &mut self.appearance),
                network::DOMAIN => &mut self.network,
                _ => continue,
            };
            mirror.apply_change(&changed);
        }
        rebuild
    }

    fn reread(&mut self, rebuild: &mut Rebuild) {
        rebuild.keybindings |= reread(&mut self.input);
        rebuild.policy |= reread(&mut self.animation);
        rebuild.policy |= reread(&mut self.windows);
        rebuild.pointer |= reread(&mut self.pointer);
        rebuild.displays |= reread(&mut self.displays);
        rebuild.policy |= reread(&mut self.ime);
        rebuild.policy |= reread(&mut self.appearance);
        reread(&mut self.network);
    }
}

fn domains(layers: &Layers) -> rikka_settings::Result<Domains> {
    let service = SettingsService::with_layers(layers.clone())
        .with_schema(keybindings::schema())?
        .with_schema(animation::schema())?
        .with_schema(windows::schema())?
        .with_schema(pointer::schema())?
        .with_schema(network::schema())?
        .with_schema(ime::schema())?;
    Ok(Domains {
        service,
        input: Settings::local_with_layers(keybindings::schema(), layers.clone())?,
        animation: Settings::local_with_layers(animation::schema(), layers.clone())?,
        windows: Settings::local_with_layers(windows::schema(), layers.clone())?,
        pointer: Settings::local_with_layers(pointer::schema(), layers.clone())?,
        displays: Settings::local_with_layers(displays::schema(&[]), layers.clone())?,
        network: Settings::local_with_layers(network::schema(), layers.clone())?,
        ime: Settings::local_with_layers(ime::schema(), layers.clone())?,
    })
}

fn reread(settings: &mut Settings) -> bool {
    if let Err(error) = settings.refresh() {
        tracing::warn!("cannot reload settings domain: {error}");
        return false;
    }
    true
}

fn rebuilt<'a>(what: &mut bool, settings: &'a mut Settings) -> &'a mut Settings {
    *what = true;
    settings
}

#[derive(Debug, Default)]
pub(crate) struct Rebuild {
    pub(crate) keybindings: bool,
    pub(crate) policy: bool,
    pub(crate) pointer: bool,
    pub(crate) displays: bool,
}

impl std::fmt::Debug for SessionSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionSettings")
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}
