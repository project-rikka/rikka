mod advice;
mod card;
mod connectors;
mod leasing;
mod link;
mod pointer;
mod rendered;
mod scanout;
mod screen;

use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::os::fd::{AsFd, OwnedFd};
use std::time::{Duration, Instant};

use rikka_boost::Booster;
use rikka_colorimetry::BlendSpace;
use rikka_compositor_input::{DeviceSettings, InputEvent, InputNotice, Leds};
use rikka_compositor_kms::{
    Connector, CrtcId, Device, Gpu, Hotplug, HotplugEvent, KmsEvent,
    OutputConfig as KmsOutputConfig, Rejection, Session, SessionEvent, Signal, Wake,
};
use rikka_compositor_planes::VrrPolicy;
use rikka_compositor_reactor::{Reactor, Sender};
use rikka_compositor_types::OutputId;
use rikka_renderer::Renderer;

use super::{Backend, Leases};
use crate::color;
use crate::error::{Error, Result};
use crate::outputs::{OutputConfig, Outputs};
use crate::programs::COMPOSITOR;
use crate::render::Snapshot;
use crate::session::NiceCapability;
use crate::state::{Compositor, Parts};

use advice::Advice;
use card::{Card, CardId};
use link::{Formats, Tier};
use pointer::Pointer;
use screen::{Drawing, Screen};

pub(crate) struct Kms {
    pointer: Pointer,
    boost: Booster,
    screens: BTreeMap<OutputId, Screen>,
    cards: BTreeMap<CardId, Card>,
    floors: BTreeMap<OutputId, Tier>,
    hdr_refused: BTreeSet<OutputId>,
    mailboxes: HashMap<CardId, Sender<KmsEvent>>,
    hotplug: Hotplug<Compositor>,
    renderer: Renderer,
    rendering: Gpu,
    formats: Formats,
    advice: Advice,
    session: Session,
    reactor: Reactor<Compositor>,
}

impl Kms {
    pub(crate) fn start(
        reactor: &Reactor<Compositor>,
        outputs: &mut Outputs,
        devices: DeviceSettings,
        nice: NiceCapability,
    ) -> Result<Self> {
        let session = Session::open(reactor, Kms::session_changed).map_err(Error::Kms)?;
        let gpus = Gpu::all(session.seat()).map_err(Error::Kms)?;
        let (renderer, rendering) = render_on(&gpus)?;
        tracing::info!("rendering on {}", rendering.path().display());
        let mut mailboxes = HashMap::new();
        let cards: BTreeMap<CardId, Card> = gpus
            .iter()
            .filter_map(|gpu| {
                let renders = gpu.device_number() == rendering.device_number();
                let card = open_card(&session, reactor, &mut mailboxes, gpu, renders)?;
                Some((card.id(), card))
            })
            .collect();
        nice.leave_state_thread();
        if cards.is_empty() {
            return Err(Error::Kms(rikka_compositor_kms::Error::NoGpu(
                session.seat().to_owned(),
            )));
        }
        let hotplug = Hotplug::watch(reactor, session.seat(), Kms::hotplug).map_err(Error::Kms)?;
        let pointer = Pointer::start(reactor, &session, devices)?;
        let mut kms = Self {
            pointer,
            boost: Booster::start(),
            screens: BTreeMap::new(),
            cards,
            floors: BTreeMap::new(),
            hdr_refused: BTreeSet::new(),
            mailboxes,
            hotplug,
            formats: Formats::from_renderer(&renderer),
            advice: Advice::new(&renderer, &rendering),
            renderer,
            rendering,
            session,
            reactor: reactor.clone(),
        };
        let cards: Vec<CardId> = kms.cards.keys().copied().collect();
        for card in cards {
            kms.rescan_card(card, outputs);
        }
        Ok(kms)
    }

    fn open(&mut self, gpu: &Gpu) -> bool {
        if self.cards.contains_key(&gpu.device_number()) {
            return false;
        }
        let renders = gpu.device_number() == self.rendering.device_number();
        let Some(card) = open_card(
            &self.session,
            &self.reactor,
            &mut self.mailboxes,
            gpu,
            renders,
        ) else {
            return false;
        };
        if renders {
            self.advice = Advice::new(&self.renderer, gpu);
        }
        self.cards.insert(card.id(), card);
        true
    }

    fn frame(compositor: &mut Compositor, output: OutputId, wake: Wake) {
        let failing = {
            let Some((kms, mut parts)) = compositor.backend::<Kms>() else {
                return;
            };
            let Some(screen) = kms.screens.get_mut(&output) else {
                return;
            };
            if !kms.session.is_active() {
                return screen.stop(&kms.reactor);
            }
            let demotable =
                |screen: &Screen| screen.failing() && screen.tier().fallback().is_some();
            if !demotable(screen) {
                let Some(card) = kms.cards.get_mut(&screen.card()) else {
                    return;
                };
                let (device, copier) = card.drawing();
                let drawing = Drawing {
                    device,
                    renderer: &mut kms.renderer,
                    copier: copier.filter(|_| screen.tier() == Tier::Copied),
                    advice: &kms.advice,
                };
                screen.draw(drawing, &mut parts, wake);
                if parts.animations.needs_frame(output) || parts.scenes.portrays_windows(output) {
                    kms.boost.touch();
                }
            }
            demotable(screen)
        };
        if failing {
            Kms::demote(compositor, output);
        }
    }

    fn demote(compositor: &mut Compositor, output: OutputId) {
        let reenabled = {
            let Some((kms, mut parts)) = compositor.backend::<Kms>() else {
                return;
            };
            let Some(screen) = kms.screens.get(&output) else {
                return;
            };
            let hdr_refused = kms.hdr_refused.contains(&output) && screen.blend_space().is_hdr();
            if !hdr_refused {
                let Some(lower) = screen.tier().fallback() else {
                    return;
                };
                tracing::warn!(
                    "{}: {} unavailable; falling back to {}",
                    screen.output().name(),
                    screen.tier().describe(),
                    lower.describe()
                );
                kms.floors.insert(output, lower);
            }
            kms.disable_output(output, &mut parts);
            let config = parts
                .outputs
                .get(output)
                .and_then(|output_state| output_state.enabled_config());
            let reenabled =
                config.is_some_and(|config| kms.enable_output(&mut parts, output, &config));
            kms.load_cursor_theme(&mut parts);
            reenabled
        };
        if !reenabled {
            compositor.displays_detected();
        }
    }

    fn kms_event(compositor: &mut Compositor, card: CardId, event: KmsEvent) {
        let (rescan, demoted) = {
            let Some((kms, mut parts)) = compositor.backend::<Kms>() else {
                return;
            };
            kms.handle_kms_event(&mut parts, card, event)
        };
        if let Some(output) = demoted {
            Kms::demote(compositor, output);
        }
        if rescan {
            Kms::rescan_connectors(compositor);
        }
    }

    fn handle_kms_event(
        &mut self,
        parts: &mut Parts<'_>,
        card: CardId,
        event: KmsEvent,
    ) -> (bool, Option<OutputId>) {
        match event {
            KmsEvent::Flipped { presented, retired } => {
                if let Some((_, screen)) = self.screen_on(card, presented.crtc) {
                    screen.presented(&presented, parts);
                    if let Some(retired) = retired {
                        screen.retired(retired.frame, retired.shown, parts);
                    }
                }
            }
            KmsEvent::Retired(retired) => {
                if let Some((_, screen)) = self.screen_on(card, retired.crtc) {
                    screen.retired(retired.frame, retired.shown, parts);
                }
            }
            KmsEvent::Rejected {
                crtc,
                frame,
                reason,
            } => {
                let Some((output, screen)) = self.screen_on(card, crtc) else {
                    tracing::warn!("{crtc} refused frame {frame}: {reason:?}");
                    return (false, None);
                };
                if reason == Rejection::Scanout {
                    screen.rejected(frame, parts);
                } else if screen.blend_space().is_hdr() && screen.awaits(frame) {
                    tracing::warn!(
                        "{} refused an HDR frame: {reason:?}; falling back to SDR",
                        screen.output().name()
                    );
                    self.hdr_refused.insert(output);
                    return (false, Some(output));
                } else if screen.tier() != Tier::Native && screen.awaits(frame) {
                    tracing::warn!(
                        "{} refused a frame {}: {reason:?}",
                        screen.output().name(),
                        screen.tier().describe()
                    );
                    screen.fail();
                    return (false, Some(output));
                } else {
                    tracing::warn!("{crtc} refused frame {frame}: {reason:?}");
                }
            }
            KmsEvent::CursorRejected { crtc } => {
                let Some((output, screen)) = self.screen_on(card, crtc) else {
                    return (false, None);
                };
                tracing::warn!("{crtc} rejected cursor plane; compositing cursor");
                screen.refuse_cursor();
                parts.outputs.set_cursor_plane(output, false);
                parts.frames.request(output);
                self.publish_route(parts);
            }
            KmsEvent::Lost { crtc } => {
                tracing::warn!("{crtc} lost; rescanning connectors");
                return (true, None);
            }
        }
        (false, None)
    }

    fn screen_on(&mut self, card: CardId, crtc: CrtcId) -> Option<(OutputId, &mut Screen)> {
        self.screens
            .iter_mut()
            .find(|(_, screen)| screen.card() == card && screen.crtc() == crtc)
            .map(|(output, screen)| (*output, screen))
    }

    fn session_changed(compositor: &mut Compositor, event: SessionEvent) {
        let resumed = {
            let Some((kms, mut parts)) = compositor.backend::<Kms>() else {
                return;
            };
            match event {
                SessionEvent::Paused => {
                    kms.pause();
                    false
                }
                SessionEvent::Resumed => {
                    kms.resume(&mut parts);
                    true
                }
            }
        };
        if resumed {
            Kms::rescan_connectors(compositor);
            compositor.frames.request_all();
        }
    }

    fn pause(&mut self) {
        self.pointer.suspend();
        for screen in self.screens.values_mut() {
            screen.stop(&self.reactor);
        }
        for card in self.cards.values() {
            if let Err(error) = card.device.pause() {
                tracing::warn!("cannot pause {}: {error}", card.gpu().path().display());
            }
        }
    }

    fn resume(&mut self, parts: &mut Parts<'_>) {
        self.pointer.resume();
        for card in self.cards.values() {
            if let Err(error) = card.device.resume() {
                tracing::warn!("cannot resume {}: {error}", card.gpu().path().display());
            }
        }
        self.load_cursor_theme(parts);
    }

    fn hotplug(compositor: &mut Compositor, event: HotplugEvent) {
        let (rescan, lost) = {
            let Some((kms, mut parts)) = compositor.backend::<Kms>() else {
                return;
            };
            match event {
                HotplugEvent::Changed(card) => {
                    (kms.cards.contains_key(&card).then_some(card), false)
                }
                HotplugEvent::Removed(card) => (None, kms.lose(card, &mut parts)),
                HotplugEvent::Added(gpu) => (kms.open(&gpu).then_some(gpu.device_number()), false),
            }
        };
        if let Some(card) = rescan {
            let changed = match compositor.backend::<Kms>() {
                Some((kms, mut parts)) => kms.rescan_one(card, &mut parts),
                None => false,
            };
            if changed {
                compositor.displays_detected();
            }
        }
        if lost {
            compositor.displays_detected();
        }
    }

    fn lose(&mut self, card: CardId, parts: &mut Parts<'_>) -> bool {
        let Some(path) = self
            .cards
            .get(&card)
            .map(|card_state| card_state.gpu().path().to_owned())
        else {
            return false;
        };
        tracing::warn!("{} removed with all its outputs", path.display());
        let gone = self
            .cards
            .get_mut(&card)
            .map(|card_state| card_state.connectors.forget(parts.outputs))
            .unwrap_or_default();
        for output in gone {
            self.disable_output(output, parts);
            self.floors.remove(&output);
            self.hdr_refused.remove(&output);
        }
        if let Some(card_state) = self.cards.remove(&card) {
            card_state.close(&self.session);
        }
        self.publish_route(parts);
        true
    }

    fn rescan_connectors(compositor: &mut Compositor) {
        let changed = match compositor.backend::<Kms>() {
            Some((kms, mut parts)) => kms.rescan(&mut parts),
            None => false,
        };
        if changed {
            compositor.displays_detected();
        }
    }

    fn input(compositor: &mut Compositor, events: &mut Vec<InputEvent>) {
        let Some((kms, mut parts)) = compositor.backend::<Kms>() else {
            return;
        };
        kms.pointer.handle_events(&mut parts, events);
    }

    fn noticed(notice: InputNotice) {
        match notice {
            InputNotice::Configured(report) => tracing::debug!(?report, "input device configured"),
            InputNotice::Failed(error) => {
                tracing::error!("input thread stopped: {error}");
            }
        }
    }

    fn rescan(&mut self, parts: &mut Parts<'_>) -> bool {
        let cards: Vec<CardId> = self.cards.keys().copied().collect();
        let mut changed = false;
        for card in cards {
            changed |= self.rescan_one(card, parts);
        }
        changed
    }

    fn rescan_one(&mut self, card: CardId, parts: &mut Parts<'_>) -> bool {
        let rescan = self.rescan_card(card, parts.outputs);
        for output in &rescan.gone {
            self.disable_output(*output, parts);
            self.floors.remove(output);
            self.hdr_refused.remove(output);
        }
        rescan.changed
    }

    fn rescan_card(&mut self, card: CardId, outputs: &mut Outputs) -> connectors::Rescan {
        let Some(mut card_state) = self.cards.remove(&card) else {
            return connectors::Rescan::default();
        };
        let used_names = OnceCell::new();
        let prefix = card_state.gpu().name().to_owned();
        let renders = card_state.renders;
        let rescan =
            card_state
                .connectors
                .rescan(&card_state.device, outputs, |connector: &Connector| {
                    let used_names = used_names.get_or_init(|| self.used_names(card, renders));
                    match used_names.contains(connector.name()) {
                        true => format!("{prefix}-{}", connector.name()),
                        false => connector.name().to_owned(),
                    }
                });
        self.cards.insert(card, card_state);
        rescan
    }

    fn used_names(&self, card: CardId, renders: bool) -> BTreeSet<String> {
        if renders {
            return BTreeSet::new();
        }
        self.cards
            .iter()
            .filter(|(id, other)| other.renders || **id < card)
            .flat_map(|(_, other)| other.device.connector_names().unwrap_or_default())
            .collect()
    }

    fn card_of(&self, output: OutputId) -> Option<CardId> {
        self.cards
            .iter()
            .find(|(_, card)| card.connectors.get(output).is_some())
            .map(|(id, _)| *id)
    }

    fn disable_output(&mut self, output: OutputId, parts: &mut Parts<'_>) {
        let Some(mut screen) = self.screens.remove(&output) else {
            return;
        };
        screen.stop(&self.reactor);
        screen.release_scanout(parts);
        parts.scenes.forget(output, &mut self.renderer);
        let card = screen.card();
        let Some(card_state) = self.cards.get_mut(&card) else {
            return;
        };
        let enabled = screen.release(card_state);
        let name = enabled.name().to_owned();
        if let Err(error) = card_state.device.disable_output(enabled) {
            tracing::warn!("cannot disable {name}: {error}");
        }
        self.release_unused_copier(card);
    }

    fn release_unused_copier(&mut self, card: CardId) {
        let copies = self
            .screens
            .values()
            .any(|screen| screen.card() == card && screen.tier() == Tier::Copied);
        if !copies && let Some(card_state) = self.cards.get_mut(&card) {
            card_state.forget_copier();
        }
    }

    fn enable_output(
        &mut self,
        parts: &mut Parts<'_>,
        output: OutputId,
        config: &OutputConfig,
    ) -> bool {
        let Some(card) = self.card_of(output) else {
            tracing::warn!("cannot enable {output:?}: no DRM device drives it");
            return false;
        };
        let Some(connector) = self
            .cards
            .get(&card)
            .and_then(|card_state| card_state.connectors.get(output))
            .cloned()
        else {
            return false;
        };
        let Some(mode) = connectors::mode(&connector, config.mode) else {
            tracing::warn!(
                "{} has no {}x{}@{}mHz mode",
                connector.name(),
                config.mode.width,
                config.mode.height,
                config.mode.refresh
            );
            return false;
        };
        let Some(head) = parts.outputs.get(output).map(|lit| lit.head().clone()) else {
            return false;
        };
        let hdr = OutputConfig {
            hdr: config.hdr && !self.hdr_refused.contains(&output),
            ..*config
        };
        let mut applied = hdr;
        let mut enabled = self.light(parts, (output, card), &connector, mode, &hdr);
        if !enabled && hdr.hdr && head.hdr.is_some() {
            tracing::warn!("{} cannot show HDR; falling back to SDR", connector.name());
            applied = OutputConfig { hdr: false, ..hdr };
            enabled = self.light(parts, (output, card), &connector, mode, &applied);
        }
        if !enabled {
            return false;
        }
        let Some(screen) = self.screens.get_mut(&output) else {
            return false;
        };
        let lit_hdr = matches!(screen.output().signal(), Signal::Pq(_));
        let applied = OutputConfig {
            hdr: lit_hdr,
            ..applied
        };
        screen.set_blend_space(color::blend_space(&head, &applied));
        screen.turn(config.transform);
        screen.set_vrr_policy(config.adaptive_sync);
        let kms_output = screen.output();
        tracing::info!(
            "{} enabled at {}x{}@{:.2}Hz on {}",
            kms_output.name(),
            kms_output.mode().width(),
            kms_output.mode().height(),
            f64::from(kms_output.mode().refresh_millihertz()) / 1000.0,
            kms_output.crtc()
        );
        parts.outputs.apply(output, applied);
        parts
            .outputs
            .set_cursor_plane(output, screen.accepts_cursor_plane());
        parts.frames.request(output);
        true
    }

    fn light(
        &mut self,
        parts: &mut Parts<'_>,
        (output, card): (OutputId, CardId),
        connector: &Connector,
        mode: rikka_compositor_kms::Mode,
        config: &OutputConfig,
    ) -> bool {
        let Some(head) = parts.outputs.get(output).map(|lit| lit.head().clone()) else {
            return false;
        };
        let signal = color::output_signal(&head, config);
        let drm_config = KmsOutputConfig::new(mode)
            .vrr(config.adaptive_sync == VrrPolicy::Always)
            .signal(signal);
        match self.screens.get(&output).map(|screen| {
            (
                screen.output().mode(),
                screen.output().signal(),
                screen.tier(),
            )
        }) {
            Some((current, lit, _)) if current == mode && lit == signal => true,
            Some((_, _, Tier::Native)) => self.modeset(output, card, connector, drm_config),
            Some(_) => {
                self.disable_output(output, parts);
                self.link(output, card, connector, drm_config)
            }
            None => self.link(output, card, connector, drm_config),
        }
    }

    fn modeset(
        &mut self,
        output: OutputId,
        card: CardId,
        connector: &Connector,
        drm_config: KmsOutputConfig,
    ) -> bool {
        let (Some(screen), Some(card)) = (self.screens.get_mut(&output), self.cards.get_mut(&card))
        else {
            return false;
        };
        let reenabled = screen.modeset(
            &mut card.device,
            connector,
            drm_config,
            self.formats.rendered(),
            &mut self.renderer,
        );
        match reenabled {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!("cannot enable {}: {error}", connector.name());
                false
            }
        }
    }

    fn link(
        &mut self,
        output: OutputId,
        card: CardId,
        connector: &Connector,
        drm_config: KmsOutputConfig,
    ) -> bool {
        let floor = self.floors.get(&output).copied().unwrap_or(Tier::Native);
        let Some(card_state) = self.cards.get_mut(&card) else {
            return false;
        };
        for tier in Tier::fallbacks(card_state.renders, floor) {
            match link::enable(
                tier,
                card_state,
                connector,
                drm_config,
                &mut self.renderer,
                &self.formats,
            ) {
                Ok(enabled) => {
                    tracing::info!("{} is {}", connector.name(), tier.describe());
                    self.floors.insert(output, tier);
                    self.screens.insert(
                        output,
                        Screen::new(output, card, tier, enabled, &mut self.renderer),
                    );
                    self.release_unused_copier(card);
                    return true;
                }
                Err(error) => {
                    tracing::info!(
                        "{} cannot be {}: {error}",
                        connector.name(),
                        tier.describe()
                    );
                }
            }
        }
        tracing::warn!("cannot enable {} with any configuration", connector.name());
        self.release_unused_copier(card);
        false
    }

    fn load_cursor_theme(&mut self, parts: &mut Parts<'_>) {
        for (output, screen) in &mut self.screens {
            let Some(card) = self.cards.get_mut(&screen.card()) else {
                continue;
            };
            let device = &mut card.device;
            if !screen.accepts_cursor_plane() {
                if screen.has_any_cursor_image() {
                    if let Err(error) = device.clear_cursor(screen.crtc()) {
                        tracing::debug!("cannot clear cursor plane: {error}");
                    }
                    screen.set_cursor_image(None);
                }
                parts.outputs.set_cursor_plane(*output, false);
                continue;
            }
            let Some(scale) = parts.outputs.get(*output).map(|output| output.scale()) else {
                continue;
            };
            if parts.cursor.hidden() || parts.cursor.surface().is_some() {
                if screen.has_any_cursor_image() {
                    if let Err(error) = device.clear_cursor(screen.crtc()) {
                        tracing::debug!("cannot clear cursor plane: {error}");
                    }
                    screen.set_cursor_image(None);
                }
                continue;
            }
            let image = parts.cursor.image(scale);
            if screen.has_cursor_image(&image) {
                continue;
            }
            let argb = match screen.blend_space() {
                BlendSpace::Sdr => image.argb(),
                BlendSpace::Hdr(space) => color::encode_cursor(&image.argb(), space),
            };
            let worn = device.set_cursor_pixels(
                screen.crtc(),
                image.width,
                image.height,
                &argb,
                image.hotspot,
            );
            match worn {
                Ok(()) => screen.set_cursor_image(Some(image)),
                Err(error) => {
                    tracing::info!(
                        "{} has no usable cursor plane: {error}",
                        screen.output().name()
                    );
                    screen.refuse_cursor();
                    parts.outputs.set_cursor_plane(*output, false);
                    parts.frames.request(*output);
                }
            }
        }
        self.publish_route(parts);
        self.pointer
            .place_over(parts.outputs, parts.cursor.position());
    }

    fn publish_route(&self, parts: &Parts<'_>) {
        let crtcs = self
            .screens
            .iter()
            .filter(|(_, screen)| screen.accepts_cursor_plane())
            .filter_map(|(output, screen)| {
                let card = self.cards.get(&screen.card())?;
                Some((*output, card.device.cursor_control(), screen.crtc()))
            })
            .collect();
        self.pointer.publish(
            parts.outputs,
            parts.input.restrictions,
            !self.cards.is_empty(),
            crtcs,
        );
    }
}

fn open_card(
    session: &Session,
    reactor: &Reactor<Compositor>,
    mailboxes: &mut HashMap<CardId, Sender<KmsEvent>>,
    gpu: &Gpu,
    renders: bool,
) -> Option<Card> {
    let id = gpu.device_number();
    let events = match mailboxes.get(&id) {
        Some(events) => events.clone(),
        None => {
            let listened = reactor.mailbox(move |compositor: &mut Compositor, event: KmsEvent| {
                Kms::kms_event(compositor, id, event);
            });
            match listened {
                Ok(events) => mailboxes.entry(id).or_insert(events).clone(),
                Err(error) => {
                    tracing::warn!("cannot watch {}: {error}", gpu.path().display());
                    return None;
                }
            }
        }
    };
    match Device::open(session, gpu, events) {
        Ok(device) => {
            tracing::info!(
                "{} drives displays{}",
                gpu.path().display(),
                if renders { " and renders" } else { "" }
            );
            Some(Card::new(device, renders))
        }
        Err(error) => {
            tracing::warn!("{} cannot drive outputs: {error}", gpu.path().display());
            None
        }
    }
}

fn render_on(gpus: &[Gpu]) -> Result<(Renderer, Gpu)> {
    let mut refusal = None;
    for gpu in gpus {
        let built = rikka_gpu::Gpu::builder()
            .app_name(COMPOSITOR)
            .drm_node(gpu.major(), gpu.minor())
            .latency_critical()
            .upload_queue()
            .build()
            .and_then(|device| Renderer::new(&device));
        match built {
            Ok(renderer) if !Formats::from_renderer(&renderer).is_empty() => {
                return Ok((renderer, gpu.clone()));
            }
            Ok(_) => {
                tracing::info!(
                    "{} cannot render into scanout buffers",
                    gpu.path().display()
                );
                refusal.get_or_insert(Error::Kms(rikka_compositor_kms::Error::NoScanoutFormat));
            }
            Err(error) => {
                tracing::info!("{} cannot render: {error}", gpu.path().display());
                refusal = Some(Error::Renderer(error));
            }
        }
    }
    Err(refusal.unwrap_or(Error::Kms(rikka_compositor_kms::Error::NoScanoutFormat)))
}

impl Backend for Kms {
    fn configure(&mut self, parts: &mut Parts<'_>, configs: &[(OutputId, OutputConfig)]) -> bool {
        if self.cards.is_empty() {
            tracing::warn!("cannot configure outputs without a GPU");
            return false;
        }
        if let Some((output, config)) = configs.iter().find(|(output, config)| {
            !self
                .cards
                .values()
                .any(|card| card.connectors.offers(*output, config))
        }) {
            tracing::warn!("{output:?} rejected {config:?}");
            return false;
        }
        let mut applied = true;
        for (output, config) in configs {
            if config.enabled {
                applied &= self.enable_output(parts, *output, config);
            } else {
                self.disable_output(*output, parts);
                parts.outputs.apply(*output, *config);
                parts.outputs.set_cursor_plane(*output, false);
            }
        }
        self.load_cursor_theme(parts);
        applied
    }

    fn schedule(&mut self, output: OutputId) {
        if !self.session.is_active() {
            return;
        }
        if let Some(screen) = self.screens.get_mut(&output) {
            screen.schedule(&self.reactor, Duration::ZERO);
        }
    }

    fn configure_devices(&mut self, settings: DeviceSettings) {
        self.pointer.configure(settings);
    }

    fn show_leds(&mut self, leds: Leds) {
        self.pointer.show_leds(leds);
    }

    fn cursor_changed(&mut self, parts: &mut Parts<'_>) {
        self.load_cursor_theme(parts);
        parts.frames.request_all();
    }

    fn switches_vt(&self) -> bool {
        true
    }

    fn renderer(&mut self) -> &mut Renderer {
        &mut self.renderer
    }

    fn rest(&mut self) {
        self.renderer.trim();
        for card in self.cards.values_mut() {
            if let Some(copier) = card.copy_renderer() {
                copier.trim();
            }
        }
    }

    fn switch_vt(&mut self, vt: i32) {
        if let Err(error) = self.session.switch_vt(vt) {
            tracing::warn!("{error}");
        }
    }

    fn route_changed(&mut self, parts: &mut Parts<'_>) {
        self.publish_route(parts);
    }

    fn pointer_warped(&mut self, parts: &mut Parts<'_>) {
        self.pointer.warped(parts);
    }

    fn gpu_reached(&mut self, finished: u64, at: Instant) {
        for screen in self.screens.values_mut() {
            screen.rendered(finished, at);
            if let Some(card) = self.cards.get(&screen.card()) {
                screen.copied(&card.device, &mut self.renderer, finished, &self.reactor);
            }
        }
    }

    fn sleep_displays(&mut self, parts: &mut Parts<'_>) -> bool {
        if !self.session.is_active() {
            return false;
        }
        let lit: Vec<OutputId> = self.screens.keys().copied().collect();
        for output in &lit {
            self.disable_output(*output, parts);
        }
        self.publish_route(parts);
        !lit.is_empty()
    }

    fn wake_displays(&mut self, parts: &mut Parts<'_>) {
        let configs: Vec<(OutputId, OutputConfig)> = parts
            .outputs
            .iter()
            .filter_map(|output| Some((output.id(), output.enabled_config()?)))
            .collect();
        for (output, config) in &configs {
            if !self.screens.contains_key(output) && !self.enable_output(parts, *output, config) {
                tracing::warn!("{output:?} did not wake");
            }
        }
        self.load_cursor_theme(parts);
        self.publish_route(parts);
    }

    fn interacted(&mut self) {
        self.boost.touch();
    }

    fn syncobj_device(&self) -> Option<OwnedFd> {
        let card = self.cards.get(&self.rendering.device_number())?;
        card.device
            .as_fd()
            .try_clone_to_owned()
            .inspect_err(|error| tracing::info!("cannot share DRM device: {error}"))
            .ok()
    }

    fn snapshot(&mut self, _: OutputId) -> Option<Snapshot> {
        None
    }

    fn leases(&mut self) -> Option<&mut dyn Leases> {
        Some(self)
    }
}

impl Drop for Kms {
    fn drop(&mut self) {
        for screen in self.screens.values_mut() {
            screen.stop(&self.reactor);
        }
        let screens = std::mem::take(&mut self.screens);
        for screen in screens.into_values() {
            if let Some(card) = self.cards.get_mut(&screen.card()) {
                drop(screen.release(card));
            }
        }
        for card in std::mem::take(&mut self.cards).into_values() {
            card.close(&self.session);
        }
    }
}

impl fmt::Debug for Kms {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Kms")
            .field("rendering", &self.rendering)
            .field("cards", &self.cards)
            .field("session", &self.session)
            .field("screens", &self.screens)
            .field("pointer", &self.pointer)
            .field("hotplug", &self.hotplug)
            .finish_non_exhaustive()
    }
}
