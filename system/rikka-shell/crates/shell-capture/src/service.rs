use std::sync::Arc;

use code_meaning::Meaning;
use rikka_clipboard::Clipboard;
use rikka_network::Network;
use rikka_record::Recording;
use rikka_seam::{
    Call, EventInfo, Manifest, Method, MethodInfo, Request, SeamError, Service, ServiceContext,
    Version,
};
use rikka_types::Rect;
use rikka_ui::tr;
use serde::{Deserialize, Serialize};
use shell_kit::{Host, Outputs};
use shell_markup::Editing;
use shell_notifications::Announcement;

use crate::aim::{Aim, Mode, Target};
use crate::deferral::{Deferrals, Deferred};
use crate::error::{Error, Result};
use crate::frozen::Frozen;
use crate::preferences::{Picture, Preferences};
use crate::recorder::{self, Subject};
use crate::scan::{self, Act, Acting, ScanResults, Scope};
use crate::shared::Shared;
use crate::shot;
use crate::snip::Snip;

pub const CAPABILITY: &str = "rikka.capture";

pub(crate) const PICTURE: &str = "picture";

pub(super) const VIDEO: &str = "video";

pub const PICK: &str = "pick";

pub(crate) const AIM: &str = "aim";

pub(crate) const CHOSE: &str = "chose";

pub(crate) const CANCEL: &str = "cancel";

pub(crate) const STOP: &str = "stop";

pub(super) const CLOSED: &str = "closed";

pub(super) const SETTLE: &str = "settle";

pub(crate) const CHANGED: &str = "changed";

pub(super) const SCANNED: &str = "scanned";

pub(super) const POINT: &str = "point";

pub(super) const ACT: &str = "act";

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub(crate) struct Area {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl From<Rect> for Area {
    fn from(rect: Rect) -> Area {
        Area {
            x: rect.min_x(),
            y: rect.min_y(),
            width: rect.width(),
            height: rect.height(),
        }
    }
}

impl From<Area> for Rect {
    fn from(area: Area) -> Rect {
        Rect::new(area.x, area.y, area.width, area.height)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Chosen {
    pub(crate) area: Area,
    pub(crate) window: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub(crate) struct Picking {
    pub(crate) shown: bool,
    pub(crate) aim: Aim,
}

pub(super) struct CaptureService {
    clipboard: Option<Arc<Clipboard>>,
    preferences: Preferences,
    shared: Shared,
    snip: Snip,
    editing: Editing,
    network: Network,
    scan_results: ScanResults,
    recording: Option<Recording>,
    deferrals: Deferrals,
    seam: Option<ServiceContext>,
}

impl CaptureService {
    #[must_use]
    pub(crate) fn new(
        clipboard: Option<Clipboard>,
        preferences: Preferences,
        shared: Shared,
        snip: Snip,
        editing: Editing,
        network: Network,
    ) -> CaptureService {
        let clipboard = clipboard.map(Arc::new);
        CaptureService {
            clipboard,
            preferences,
            shared,
            snip,
            editing,
            network,
            scan_results: ScanResults::default(),
            recording: None,
            deferrals: Deferrals::default(),
            seam: None,
        }
    }

    fn freeze(outputs: &Outputs, picture: &Picture, mode: Mode) -> Result<Arc<Frozen>> {
        let cursor = picture.cursor && mode != Mode::Scan;
        Ok(Arc::new(Frozen::take(&outputs.all(), cursor)?))
    }

    fn request(&mut self, outputs: &Outputs, call: Call, deferred: Deferred) {
        if self.deferrals.is_closing() {
            self.deferrals.hold(deferred);
            return call.done();
        }
        match self.perform(outputs, deferred) {
            Ok(()) => call.done(),
            Err(error) => call.fail(refused(&error)),
        }
    }

    fn closed(&mut self, outputs: &Outputs) {
        for deferred in self.deferrals.release() {
            if let Err(error) = self.perform(outputs, deferred) {
                tracing::warn!("failed to capture once the picker closed: {error}");
            }
        }
    }

    fn perform(&mut self, outputs: &Outputs, deferred: Deferred) -> Result<()> {
        match deferred {
            Deferred::Picture => self.picture(outputs),
            Deferred::Video => self.video(outputs),
            Deferred::Pick(mode) => self.pick(outputs, mode),
            Deferred::Record(subject) => self.begin(subject),
        }
    }

    fn picture(&mut self, outputs: &Outputs) -> Result<()> {
        if self.shared.picking.is_shown() {
            return Err(Error::Busy);
        }
        let picture = self.preferences.picture();
        let frozen = Self::freeze(outputs, &picture, Mode::Picture)?;
        let region = frozen.bounds();
        self.keep(frozen, region, picture);
        Ok(())
    }

    fn toggle_video(&mut self, outputs: &Outputs, call: Call) {
        if self.recording.is_some() {
            self.end();
            return call.done();
        }
        self.request(outputs, call, Deferred::Video);
    }

    fn video(&mut self, outputs: &Outputs) -> Result<()> {
        if self.shared.picking.is_shown() {
            return Err(Error::Busy);
        }
        let name = self
            .shared
            .windows
            .focused_output()
            .or_else(|| Some(outputs.all().first()?.name.clone()))
            .unwrap_or_default();
        self.begin(Subject::Output { name, part: None })
    }

    fn requested_pick(&mut self, outputs: &Outputs, call: Call) {
        match call.decode::<Mode>() {
            Ok(mode) => self.request(outputs, call, Deferred::Pick(mode)),
            Err(error) => call.fail(error),
        }
    }

    fn pick(&mut self, outputs: &Outputs, mode: Mode) -> Result<()> {
        let aim = Aim {
            mode,
            ..self.shared.aimed.get()
        };
        if self.shared.picking.is_shown() {
            self.aimed(aim);
            return Ok(());
        }
        let frozen = Self::freeze(outputs, &self.preferences.picture(), mode)?;
        self.shared.aimed.set(aim);
        self.shared.selection.clear();
        self.shared.codes.clear();
        *self.shared.frozen.borrow_mut() = Some(frozen);
        if mode == Mode::Scan {
            self.screen();
        }
        self.show(true);
        Ok(())
    }

    fn aim(&mut self, call: Call) {
        match call.decode::<Aim>() {
            Ok(aim) => {
                self.aimed(aim);
                call.done();
            }
            Err(error) => call.fail(error),
        }
    }

    fn chose(&mut self, call: Call) {
        let chosen = match call.decode::<Chosen>() {
            Ok(chosen) => chosen,
            Err(error) => return call.fail(error),
        };
        let mode = self.shared.aimed.mode();
        if mode == Mode::Scan {
            self.scan(self.shared.codes.frame(Rect::from(chosen.area)));
            self.announce();
            return call.done();
        }
        let frozen = self.shared.frozen.borrow().clone();
        self.show(false);
        let Some(frozen) = frozen else {
            return call.fail(unavailable(
                "the screen is not frozen; there is nothing aimed at",
            ));
        };

        let region = Rect::from(chosen.area);
        let began = match mode {
            Mode::Picture => {
                self.keep(frozen, region, self.preferences.picture());
                Ok(())
            }
            Mode::Video => match self.subject(&frozen, region, chosen.window) {
                Some(subject) => {
                    self.deferrals.hold(Deferred::Record(subject));
                    Ok(())
                }
                None => Err(Error::NothingSelected),
            },
            Mode::Translate => {
                shot::snip(frozen, region, self.snip.clone(), self.shared.seam.clone());
                Ok(())
            }
            Mode::Scan => Ok(()),
        };
        match began {
            Ok(()) => call.done(),
            Err(error) => call.fail(refused(&error)),
        }
    }

    fn subject(&self, frozen: &Frozen, region: Rect, window: String) -> Option<Subject> {
        if !window.is_empty() {
            return Some(Subject::Window(window));
        }
        let aimed = frozen.aim(region)?;
        Some(Subject::Output {
            name: aimed.output,
            part: (!aimed.whole).then_some(aimed.region),
        })
    }

    fn keep(&self, frozen: Arc<Frozen>, region: Rect, picture: Picture) {
        shot::spawn(shot::ShotRequest {
            frozen,
            region,
            picture,
            clipboard: self.clipboard.clone(),
            editing: self.editing.clone(),
            words: picture_words(),
            seam: self.shared.seam.clone(),
        });
    }

    fn begin(&mut self, subject: Subject) -> Result<()> {
        if self.recording.is_some() {
            return Err(Error::AlreadyRecording);
        }
        let words = video_words();
        match recorder::start(
            &subject,
            &self.preferences.video(),
            self.shared.seam.clone(),
        ) {
            Ok((recording, file)) => {
                self.recording = Some(recording);
                self.shared.recording.began();
                recorder::began(&words, &file).send(&self.shared.seam);
                Ok(())
            }
            Err(error) => {
                Announcement::new(&words.app_name, &words.failed)
                    .body(error.to_string())
                    .urgent()
                    .send(&self.shared.seam);
                Err(error)
            }
        }
    }

    fn end(&mut self) {
        let Some(recording) = self.recording.take() else {
            return;
        };
        self.shared.recording.ended();
        recorder::finish(recording, video_words(), self.shared.seam.clone());
    }

    fn settle(&mut self) {
        if self.recording.as_ref().is_some_and(Recording::is_finished) {
            self.end();
        }
    }

    fn aimed(&mut self, aim: Aim) {
        if aim.target != Target::Region || aim.mode == Mode::Scan {
            self.shared.selection.clear();
        }
        self.shared.aimed.set(aim);
        if aim.mode == Mode::Scan && self.shared.picking.is_shown() {
            self.screen();
        }
        self.announce();
    }

    fn screen(&self) {
        if let Some(scope) = self.shared.codes.screen() {
            self.scan(scope);
        }
    }

    fn scan(&self, scope: Scope) {
        let Some(frozen) = self.shared.frozen.borrow().clone() else {
            return;
        };
        scan::look(
            frozen,
            scope,
            self.scan_results.clone(),
            self.shared.seam.clone(),
        );
    }

    fn scanned(&mut self) {
        let current = self.shared.frozen.borrow().clone();
        let before = self.shared.codes.read().chosen;
        let mut changed = false;
        for scan_result in self.scan_results.take() {
            let fresh = current
                .as_ref()
                .is_some_and(|frozen| Arc::ptr_eq(frozen, &scan_result.frozen));
            if !fresh {
                continue;
            }
            changed |= match scan_result.read {
                Ok(codes) => self.shared.codes.take_in(scan_result.scope, codes),
                Err(_) => self.shared.codes.failed(scan_result.scope),
            };
        }
        if changed {
            self.rescan_codes(before);
            self.announce();
        }
    }

    fn point(&mut self, call: Call) {
        let chosen = match call.decode::<Option<usize>>() {
            Ok(chosen) => chosen,
            Err(error) => return call.fail(error),
        };
        let before = self.shared.codes.read().chosen;
        self.shared.codes.choose(chosen);
        self.rescan_codes(before);
        self.announce();
        call.done();
    }

    fn rescan_codes(&self, before: Option<usize>) {
        let wants_networks = {
            let decoded = self.shared.codes.read();
            decoded.chosen != before
                && decoded
                    .chosen()
                    .is_some_and(|(_, code)| matches!(code.meaning, Meaning::Network(_)))
        };
        if wants_networks {
            self.network.look();
        }
    }

    fn act(&mut self, call: Call) {
        let acting = match call.decode::<Acting>() {
            Ok(acting) => acting,
            Err(error) => return call.fail(error),
        };
        let Some(code) = self.shared.codes.code(acting.code) else {
            return call.fail(SeamError::InvalidParams(format!(
                "no code {} was found on the screen",
                acting.code
            )));
        };
        let seam = &self.shared.seam;
        match (acting.act, &code.meaning) {
            (Act::Copy, _) => scan::acting::copy(&code, self.clipboard.clone(), seam.clone()),
            (Act::Open, Meaning::Link(link)) => scan::acting::open(link.address(), seam),
            (Act::Settings, Meaning::Network(_)) => scan::acting::settings(seam),
            (Act::Join, Meaning::Network(invitation)) => match self.network.accept(invitation) {
                Ok(joining) => {
                    scan::joining::follow(self.network.clone(), joining, seam.clone());
                }
                Err(trouble) => {
                    self.shared.codes.troubled(trouble);
                    self.announce();
                    return call.done();
                }
            },
            (act, _) => {
                return call.fail(SeamError::InvalidParams(format!(
                    "code {} cannot be acted on with {act:?}",
                    acting.code
                )));
            }
        }
        self.show(false);
        call.done();
    }

    fn show(&mut self, picking: bool) {
        if !picking {
            self.shared.selection.clear();
            self.shared.codes.clear();
            *self.shared.frozen.borrow_mut() = None;
        }
        if self.shared.picking.set(picking) {
            if !picking {
                self.close();
            }
            self.announce();
        }
    }

    fn close(&mut self) {
        self.deferrals.close();
        let seam = self.shared.seam.clone();
        self.shared
            .picking
            .after_unmap(move || seam.notify(Request::new(CAPABILITY, CLOSED)));
    }

    fn announce(&self) {
        let picking = Picking {
            shown: self.shared.picking.is_shown(),
            aim: self.shared.aimed.get(),
        };
        if let Some(seam) = &self.seam
            && let Err(error) = seam.publisher().emit(CHANGED, &picking)
        {
            tracing::warn!("failed to emit picker change: {error}");
        }
    }
}

impl Service<Host> for CaptureService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, Version::new(1, 0))
            .summary("Screenshots and screen recording")
            .method(MethodInfo::new(PICTURE).summary("Captures every output"))
            .method(
                MethodInfo::new(VIDEO)
                    .summary("Starts recording the focused output, or stops the running recording"),
            )
            .method(
                MethodInfo::new(PICK)
                    .summary("Freezes the screen and opens the capture picker in `mode`"),
            )
            .method(MethodInfo::new(AIM).summary("Sets the picker target"))
            .method(
                MethodInfo::new(CHOSE)
                    .summary("Captures `area` of the frozen screen, or the named `window`"),
            )
            .method(MethodInfo::new(CANCEL).summary("Cancels the capture picker"))
            .method(MethodInfo::new(STOP).summary("Stops the running recording"))
            .method(
                MethodInfo::new(CLOSED)
                    .summary("The picker is gone; runs the captures that waited for it"),
            )
            .method(MethodInfo::new(SETTLE).summary("The recording ended on its own"))
            .method(
                MethodInfo::new(SCANNED).summary("Delivers codes decoded from the frozen screen"),
            )
            .method(
                MethodInfo::new(POINT)
                    .summary("Returns the decoded code at `index` on the frozen screen"),
            )
            .method(MethodInfo::new(ACT).summary(
                "Acts on the `code` at an index: `open`s its link, `copy`s it, `join`s its \
                 network or opens Wi-Fi `settings`",
            ))
            .event(
                EventInfo::new(CHANGED)
                    .summary("Picker state changed")
                    .latest(),
            )
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.seam = Some(seam);
    }

    fn call(&mut self, host: &mut Host, method: Method, call: Call) {
        match &*method {
            PICTURE => self.request(host.outputs(), call, Deferred::Picture),
            VIDEO => self.toggle_video(host.outputs(), call),
            PICK => self.requested_pick(host.outputs(), call),
            AIM => self.aim(call),
            CHOSE => self.chose(call),
            CANCEL => {
                self.show(false);
                call.done();
            }
            STOP => {
                self.end();
                call.done();
            }
            CLOSED => {
                self.closed(host.outputs());
                call.done();
            }
            SETTLE => {
                self.settle();
                call.done();
            }
            SCANNED => {
                self.scanned();
                call.done();
            }
            POINT => self.point(call),
            ACT => self.act(call),
            _ => call.unknown_method(),
        }
    }
}

impl std::fmt::Debug for CaptureService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaptureService")
            .field("picking", &self.shared.picking.is_shown())
            .field("recording", &self.recording.is_some())
            .field("copies", &self.clipboard.is_some())
            .finish_non_exhaustive()
    }
}

fn picture_words() -> shot::Strings {
    shot::Strings {
        app_name: tr!("shell.capture.picture.name").to_string(),
        saved: tr!("shell.capture.picture.saved").to_string(),
        copied: tr!("shell.capture.picture.copied").to_string(),
        failed: tr!("shell.capture.picture.failed").to_string(),
    }
}

fn video_words() -> recorder::Strings {
    recorder::Strings {
        app_name: tr!("shell.capture.video.name").to_string(),
        started: tr!("shell.capture.video.started").to_string(),
        saved: tr!("shell.capture.video.saved").to_string(),
        failed: tr!("shell.capture.video.failed").to_string(),
    }
}

fn unavailable(reason: &str) -> SeamError {
    tracing::warn!("capture failed: {reason}");
    SeamError::Unavailable(reason.to_owned())
}

fn refused(error: &Error) -> SeamError {
    unavailable(&error.to_string())
}
