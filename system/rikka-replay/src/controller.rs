use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

use rikka_seam::{Call, Publisher, SeamError};
use rikka_session::replay::{BufferedSubject, CHANGED, Draft, Keep, ReplayStatus, Save, Saved};
use rikka_session::toplevels::{State, Toplevel};
use rikka_settings::seam::{Announcement, Settings};

use crate::clock;
use crate::engine::{Engine, WindowInfo, display_subject, window_subject};
use crate::error::{Error, Result};
use crate::folder::{self, Drafts};
use crate::options::Options;
use crate::saving::{self, Recording};
use crate::sound::Sound;
use crate::store::{DESKTOP, Entry, MICROPHONE, SharedStore, Store};

pub(crate) enum Message {
    Call(Box<Call>),
    Settings(Announcement),
    Toplevels(State),
    StateChanged,
    SessionEnded,
}

pub(crate) struct Controller {
    sound: Option<Sound>,
    engine: Option<Engine>,
    settings: Settings,
    options: Options,
    store: SharedStore,
    toplevels: Vec<Toplevel>,
    recent: Vec<String>,
    messages: Sender<Message>,
    publisher: Option<Publisher>,
    drafts: Option<Drafts>,
    draft: Option<PathBuf>,
    draft_serial: Arc<AtomicU64>,
}

impl Controller {
    pub(crate) fn new(settings: Settings, messages: Sender<Message>) -> Controller {
        let drafts = Drafts::open()
            .inspect_err(|error| tracing::warn!(%error, "replay drafts unavailable"))
            .ok();
        let options = Options::read(&settings);
        let mut controller = Controller {
            sound: None,
            engine: None,
            store: SharedStore::new(options.length, options.budget),
            settings,
            options: options.clone(),
            toplevels: Vec::new(),
            recent: Vec::new(),
            messages,
            publisher: None,
            drafts,
            draft: None,
            draft_serial: Arc::new(AtomicU64::new(0)),
        };
        controller.apply(options, true);
        controller
    }

    pub(crate) fn publish_with(&mut self, publisher: Publisher) {
        self.publisher = Some(publisher);
    }

    pub(crate) fn settings_changed(&mut self, announcement: Announcement) {
        match announcement {
            Announcement::Changed(changed) => {
                if !self.settings.apply_change(&changed) {
                    return;
                }
            }
            Announcement::Lagged => {
                if let Err(error) = self.settings.refresh() {
                    tracing::warn!("cannot reload settings: {error}");
                }
            }
        }
        let next = Options::read(&self.settings);
        self.apply(next, false);
    }

    pub(crate) fn toplevels(&mut self, state: State) {
        if let Some(focused) = state.toplevels.iter().find(|toplevel| toplevel.focused) {
            self.recent.retain(|id| *id != focused.id);
            self.recent.insert(0, focused.id.clone());
        }
        self.recent
            .retain(|id| state.toplevels.iter().any(|toplevel| toplevel.id == *id));
        self.toplevels = state.toplevels;
        if let Some(engine) = &self.engine {
            engine.windows(self.window_info());
        }
    }

    pub(crate) fn publish_status(&self) {
        let Some(publisher) = &self.publisher else {
            return;
        };
        if let Err(error) = publisher.emit(CHANGED, &self.status()) {
            tracing::debug!("cannot publish status: {error}");
        }
    }

    pub(crate) fn status(&self) -> ReplayStatus {
        let now = clock::now_us();
        let keep = clock::micros(self.options.length);
        let store = self.store.lock();
        let mut subjects: Vec<BufferedSubject> = Vec::new();
        for entry in store.entries().iter().rev() {
            if subjects.iter().any(|listed| listed.id == entry.subject) {
                continue;
            }
            let Some(span) = store.ring.span(entry.pictures) else {
                continue;
            };
            let end = match span.ended {
                true => span.to_us.min(now),
                false => now,
            };
            subjects.push(BufferedSubject {
                id: entry.subject.clone(),
                kind: entry.kind.clone(),
                seconds: (end - span.from_us.max(now - keep)).max(0) as f64 / 1e6,
                ended: span.ended,
            });
        }
        subjects.reverse();
        ReplayStatus {
            running: self.engine.is_some(),
            subjects,
        }
    }

    pub(crate) fn save(&self, call: Call) {
        let save: Save = match call.decode() {
            Ok(save) => save,
            Err(error) => return call.fail(error),
        };
        let recording = match self.prepare_recording(save.subject, save.seconds) {
            Ok(recording) => recording,
            Err(error) => return call.fail(to_seam_error(&error)),
        };
        let folder = self.options.folder.clone();
        let (_, responder) = call.defer();
        saving::in_background(move || {
            let saved = folder::output_folder(folder.as_deref())
                .and_then(|folder| {
                    folder::write_new_file(&folder, |file| saving::write(&recording, file))
                })
                .map(|(file, ())| Saved {
                    file,
                    seconds: recording.seconds(),
                });
            match saved {
                Ok(saved) => {
                    tracing::info!(
                        "saved {:.1} s replay to {}",
                        saved.seconds,
                        saved.file.display()
                    );
                    responder.encode(&saved);
                }
                Err(error) => responder.fail(to_seam_error(&error)),
            }
        });
    }

    pub(crate) fn draft(&mut self, call: Call) {
        let subject = match call.decode::<rikka_session::replay::DraftRequest>() {
            Ok(draw) => draw.subject,
            Err(error) => return call.fail(error),
        };
        let recording = match self.prepare_recording(Some(subject), None) {
            Ok(recording) => recording,
            Err(error) => return call.fail(to_seam_error(&error)),
        };
        let Some(drafts) = &self.drafts else {
            return call.fail(to_seam_error(&Error::NoFolder));
        };
        let needed = recording.bytes() + recording.bytes() / 16;
        if let Some(available) = drafts.available_bytes()
            && available < needed
        {
            let error = Error::InsufficientSpace {
                available: available >> 20,
                needed: needed >> 20,
            };
            return call.fail(to_seam_error(&error));
        }
        let serial = self.draft_serial.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(old) = self.draft.take() {
            folder::remove_file(&old);
        }
        let file = drafts.file(serial);
        self.draft = Some(file.clone());
        let latest = Arc::clone(&self.draft_serial);
        let (_, responder) = call.defer();
        saving::in_background(move || {
            let written = saving::write(&recording, &file).and_then(|()| {
                if latest.load(Ordering::SeqCst) == serial {
                    return Ok(());
                }
                folder::remove_file(&file);
                Err(Error::Superseded)
            });
            match written {
                Ok(()) => responder.encode(&Draft {
                    file,
                    seconds: recording.seconds(),
                }),
                Err(error) => responder.fail(to_seam_error(&error)),
            }
        });
    }

    pub(crate) fn keep(&self, call: Call) {
        let keep: Keep = match call.decode() {
            Ok(keep) => keep,
            Err(error) => return call.fail(error),
        };
        if !self.holds_draft(&keep.draft) {
            return call.fail(to_seam_error(&Error::UnknownDraft(keep.draft)));
        }
        let folder = self.options.folder.clone();
        let (_, responder) = call.defer();
        saving::in_background(move || {
            let span = std::time::Duration::from_secs_f64(keep.from.max(0.0))
                ..std::time::Duration::from_secs_f64(keep.to.max(keep.from).max(0.0));
            let saved = folder::output_folder(folder.as_deref())
                .and_then(|folder| {
                    folder::write_new_file(&folder, |file| {
                        Ok(rikka_video::trim(&keep.draft, file, span)?)
                    })
                })
                .map(|(file, trimmed)| Saved {
                    file,
                    seconds: (trimmed.end - trimmed.start).as_secs_f64(),
                });
            match saved {
                Ok(saved) => responder.encode(&saved),
                Err(error) => responder.fail(to_seam_error(&error)),
            }
        });
    }

    pub(crate) fn discard(&mut self, call: Call) {
        let draft: PathBuf = match call.decode() {
            Ok(draft) => draft,
            Err(error) => return call.fail(error),
        };
        if !self.holds_draft(&draft) {
            return call.fail(to_seam_error(&Error::UnknownDraft(draft)));
        }
        folder::remove_file(&draft);
        if self.draft.as_ref() == Some(&draft) {
            self.draft = None;
        }
        call.done();
    }

    fn holds_draft(&self, path: &Path) -> bool {
        self.drafts
            .as_ref()
            .is_some_and(|drafts| drafts.contains_draft(path))
    }

    fn apply(&mut self, next: Options, first: bool) {
        let was = std::mem::replace(&mut self.options, next.clone());
        if !next.enabled {
            if self.engine.is_some() || self.sound.is_some() || first {
                tracing::info!("instant replay disabled");
            }
            self.engine = None;
            self.sound = None;
            self.store.lock().clear();
            rikka_heap::return_free_glibc_memory();
            self.publish_status();
            return;
        }
        self.store.lock().ring.set_limits(next.length, next.budget);
        match &self.engine {
            Some(engine) if was.capture != next.capture => engine.set_capture(next.capture.clone()),
            Some(_) => {}
            None => {
                tracing::info!(
                    "instant replay enabled: last {} s, at most {} MiB",
                    next.length.as_secs(),
                    next.budget >> 20
                );
                let messages = self.messages.clone();
                let on_change = Box::new(move || {
                    let _ = messages.send(Message::StateChanged);
                });
                match Engine::start(
                    next.capture.clone(),
                    self.window_info(),
                    self.store.clone(),
                    on_change,
                ) {
                    Ok(engine) => self.engine = Some(engine),
                    Err(error) => tracing::error!("cannot start capture: {error}"),
                }
            }
        }
        if self.sound.is_none() || was.audio != next.audio {
            self.sound = None;
            let capturing_audio = next.audio.desktop || next.audio.microphone.is_some();
            if capturing_audio {
                match Sound::start(next.audio.clone(), self.store.clone()) {
                    Ok(sound) => self.sound = Some(sound),
                    Err(error) => tracing::warn!("cannot start audio capture: {error}"),
                }
            }
            if !next.audio.desktop {
                self.store.lock().set_sound_stream(DESKTOP, None);
            }
            if next.audio.microphone.is_none() {
                self.store.lock().set_sound_stream(MICROPHONE, None);
            }
        }
        self.publish_status();
    }

    fn window_info(&self) -> Vec<WindowInfo> {
        let mut window_info: Vec<WindowInfo> = self
            .recent
            .iter()
            .filter_map(|id| self.toplevels.iter().find(|toplevel| toplevel.id == *id))
            .chain(
                self.toplevels
                    .iter()
                    .filter(|toplevel| !self.recent.contains(&toplevel.id)),
            )
            .map(|toplevel| WindowInfo {
                id: toplevel.id.clone(),
                output: toplevel.output.clone(),
            })
            .collect();
        window_info.dedup_by(|a, b| a.id == b.id);
        window_info
    }

    fn prepare_recording(
        &self,
        subject: Option<String>,
        seconds: Option<f64>,
    ) -> Result<Recording> {
        if !self.options.enabled {
            return Err(Error::Disabled);
        }
        let store = self.store.lock();
        let entry = self.select_entry(&store, subject)?;
        let now = clock::now_us();
        let keep = clock::micros(self.options.length);
        let span = store.ring.span(entry.pictures).ok_or(Error::Empty)?;
        let from = match seconds {
            Some(seconds) => {
                now - clock::micros(std::time::Duration::from_secs_f64(seconds.max(0.0))).min(keep)
            }
            None => (now - keep).max(span.from_us),
        };
        let clip = store
            .ring
            .clip(entry.pictures, from..now)
            .ok_or(Error::Empty)?;
        Ok(Recording {
            clip,
            pictures: entry.stream.clone(),
            sounds: vec![store.sound(DESKTOP), store.sound(MICROPHONE)],
        })
    }

    fn select_entry(&self, store: &Store, requested: Option<String>) -> Result<Entry> {
        if let Some(requested) = requested {
            return store
                .latest(&requested)
                .cloned()
                .ok_or(Error::UnknownSubject(requested));
        }
        let windows = self
            .recent
            .iter()
            .map(|id| window_subject(id))
            .find_map(|subject| store.latest(&subject).cloned());
        let focused_window = self
            .toplevels
            .iter()
            .find(|toplevel| toplevel.focused)
            .and_then(|toplevel| store.latest(&window_subject(&toplevel.id)).cloned());
        let focused_display = self
            .toplevels
            .iter()
            .find(|toplevel| toplevel.focused)
            .and_then(|toplevel| store.latest(&display_subject(&toplevel.output)).cloned());
        focused_window
            .or(focused_display)
            .or(windows)
            .or_else(|| store.entries().last().cloned())
            .ok_or(Error::Empty)
    }
}

fn to_seam_error(error: &Error) -> SeamError {
    tracing::warn!("{error}");
    SeamError::unavailable(error)
}
