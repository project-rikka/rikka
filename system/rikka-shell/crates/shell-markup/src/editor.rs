use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use rikka_image::{Format, Image};
use rikka_image_edit::{Imaging, Mark, Marks, Recipe, Sketch};
use rikka_seam::Request;
use rikka_session::following;
use rikka_types::{Color, Size};
use rikka_ui::{ImageSource, State, skip_view, spawn_blocking, stream};
use shell_kit::{CLOSE, Spot, copy_picture_to_clipboard, pictures};

use crate::editing::Opening;
use crate::error::{Error, Result};
use crate::service::{ARRIVED, CAPABILITY};
use crate::shared::{Picked, Shared};
use crate::tools::{Pick, Width};

const GPU_PATIENCE: Duration = Duration::from_secs(5);

pub(crate) struct Shown {
    pub(crate) picture: Arc<Image>,
    pub(crate) source: ImageSource,
}

impl Shown {
    #[must_use]
    pub(crate) fn natural(&self) -> Size {
        let (width, height) = self.picture.dimensions();
        #[expect(
            clippy::cast_precision_loss,
            reason = "no screenshot is wider than 2^24 pixels"
        )]
        Size::new(width as f32, height as f32)
    }
}

#[derive(State)]
pub(crate) struct Editor {
    shared: Shared,
    pending_open: Option<Opening>,
    opening: u64,
    shown: Option<Shown>,
    sketch: Rc<RefCell<Sketch>>,
    saved: u64,
    trouble: Option<Arc<Error>>,
    working: bool,
}

impl Editor {
    #[must_use]
    pub(crate) fn new(shared: Shared) -> Editor {
        Editor {
            shared,
            pending_open: None,
            opening: 0,
            shown: None,
            sketch: Rc::default(),
            saved: 0,
            trouble: None,
            working: false,
        }
    }

    pub(crate) fn start(&mut self) {
        let arrived = following::changes(&self.shared.seam, CAPABILITY, ARRIVED);
        stream(arrived, |editor: &mut Editor, ()| editor.arrived());
        self.arrived();
    }

    #[must_use]
    pub(crate) fn shown(&self) -> Option<&Shown> {
        self.shown.as_ref()
    }

    #[must_use]
    pub(super) fn sketch(&self) -> Rc<RefCell<Sketch>> {
        Rc::clone(&self.sketch)
    }

    #[must_use]
    pub(crate) fn picked(&self) -> Picked {
        self.shared.choices.get()
    }

    #[must_use]
    pub(crate) fn trouble(&self) -> Option<&Error> {
        self.trouble.as_deref()
    }

    #[must_use]
    pub(crate) fn spot(&self) -> &Spot {
        &self.shared.spot
    }

    #[must_use]
    pub(crate) fn is_working(&self) -> bool {
        self.working
    }

    #[must_use]
    pub(crate) fn is_unsaved(&self) -> bool {
        self.sketch.borrow().revision() != self.saved
    }

    #[must_use]
    pub(crate) fn can_undo(&self) -> bool {
        self.sketch.borrow().can_undo()
    }

    #[must_use]
    pub(crate) fn can_redo(&self) -> bool {
        self.sketch.borrow().can_redo()
    }

    fn arrived(&mut self) {
        if !self.shared.waiting.is_waiting() {
            return skip_view();
        }
        let Some(opening) = self.shared.waiting.take() else {
            return skip_view();
        };
        let token = opening.token;
        let file = opening.file.clone();
        self.take(opening);
        let mark = self.opening;
        if let Some(picture) = self.shared.editing.picture(token) {
            return self.finish_open(mark, Ok(picture));
        }
        self.working = true;
        spawn_blocking(
            move || Image::open(&file).map(Arc::new).map_err(Error::from),
            move |editor: &mut Editor, finish_open: Result<Arc<Image>>| {
                editor.finish_open(mark, finish_open.map_err(Arc::new));
            },
        );
    }

    fn take(&mut self, opening: Opening) {
        self.pending_open = Some(opening);
        self.opening = self.opening.wrapping_add(1);
        self.shown = None;
        self.sketch = Rc::default();
        self.saved = self.sketch.borrow().revision();
        self.trouble = None;
        self.working = false;
    }

    fn finish_open(&mut self, mark: u64, finish_open: Result<Arc<Image>, Arc<Error>>) {
        if mark != self.opening {
            return skip_view();
        }
        self.working = false;
        match finish_open {
            Ok(picture) => {
                let (width, height) = picture.dimensions();
                self.shown = Some(Shown {
                    source: ImageSource::Rgba {
                        width,
                        height,
                        pixels: Arc::from(picture.rgba8()),
                    },
                    picture,
                });
            }
            Err(error) => {
                tracing::warn!("cannot edit screenshot: {error}");
                self.trouble = Some(error);
            }
        }
    }

    pub(crate) fn pick(&mut self, pick: Pick) {
        self.choose(|picked| picked.pick = pick);
    }

    pub(crate) fn ink(&mut self, ink: Color) {
        self.choose(|picked| picked.ink = ink);
    }

    pub(super) fn widen(&mut self, width: Width) {
        self.choose(|picked| picked.width = width);
    }

    fn choose(&mut self, choose: impl FnOnce(&mut Picked)) {
        let current = self.shared.choices.get();
        let mut picked = current;
        choose(&mut picked);
        if picked == current {
            return skip_view();
        }
        self.shared.choices.set(picked);
    }

    pub(crate) fn undo(&mut self) {
        if !self.sketch.borrow_mut().undo() {
            skip_view();
        }
    }

    pub(crate) fn redo(&mut self) {
        if !self.sketch.borrow_mut().redo() {
            skip_view();
        }
    }

    pub(crate) fn save(&mut self) {
        if !self.is_unsaved() {
            return skip_view();
        }
        let (Some(shown), Some(pending_open)) = (self.shown.as_ref(), self.pending_open.as_ref())
        else {
            return skip_view();
        };
        if self.working {
            return skip_view();
        }
        let picture = Arc::clone(&shown.picture);
        let marks = self.sketch.borrow().marks().to_vec();
        let file = pending_open.file.clone();
        let quality = pending_open.quality;
        let revision = self.sketch.borrow().revision();
        let mark = self.opening;
        self.working = true;
        self.trouble = None;
        spawn_blocking(
            move || write(&picture, &marks, &file, quality).map(|()| revision),
            move |editor: &mut Editor, result: Result<u64>| {
                editor.saved(mark, result.map_err(Arc::new));
            },
        );
    }

    fn saved(&mut self, mark: u64, result: Result<u64, Arc<Error>>) {
        if mark != self.opening {
            return skip_view();
        }
        self.working = false;
        match result {
            Ok(revision) => self.saved = revision,
            Err(error) => {
                tracing::warn!("failed to write edited screenshot: {error}");
                self.trouble = Some(error);
            }
        }
    }

    pub(crate) fn copy(&mut self) {
        let Some(shown) = self.shown.as_ref() else {
            return skip_view();
        };
        if self.working {
            return skip_view();
        }
        let picture = Arc::clone(&shown.picture);
        let marks = self.sketch.borrow().marks().to_vec();
        let mark = self.opening;
        self.working = true;
        self.trouble = None;
        spawn_blocking(
            move || encode(&picture, &marks),
            move |editor: &mut Editor, encoded: Result<Arc<[u8]>>| {
                editor.encoded(mark, encoded.map_err(Arc::new));
            },
        );
    }

    fn encoded(&mut self, mark: u64, encoded: Result<Arc<[u8]>, Arc<Error>>) {
        if mark != self.opening {
            return skip_view();
        }
        self.working = false;
        match encoded {
            Ok(png) => copy_picture_to_clipboard(png),
            Err(error) => {
                tracing::warn!("failed to copy edited screenshot: {error}");
                self.trouble = Some(error);
            }
        }
    }

    pub(crate) fn close(&mut self) {
        match self.shared.waiting.is_waiting() {
            true => self.arrived(),
            false => {
                self.shared.seam.notify(Request::new(CAPABILITY, CLOSE));
            }
        }
    }
}

fn flatten(picture: &Image, marks: &[Mark]) -> Result<Image> {
    let compute = rikka_picture::SharedGpu::compute(GPU_PATIENCE).ok_or(Error::NoGpu)?;
    let imaging = Imaging::shared(&compute);
    let drawn = imaging
        .picture(picture)?
        .develop(&Recipe::new().then(Marks::new(marks.iter().cloned())))?;
    Ok(drawn.to_image()?)
}

fn write(picture: &Image, marks: &[Mark], file: &Path, quality: u8) -> Result<()> {
    let flattened = flatten(picture, marks)?;
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder).map_err(Error::NoFolder)?;
    }
    let format = Format::from_path(file).unwrap_or(Format::Png);
    flattened.write(format).quality(quality).to(file)?;
    tracing::info!("wrote edited screenshot to {}", file.display());
    Ok(())
}

fn encode(picture: &Image, marks: &[Mark]) -> Result<Arc<[u8]>> {
    let flattened = flatten(picture, marks)?;
    let png = pictures::png(&flattened)?;
    Ok(Arc::from(png))
}

impl std::fmt::Debug for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor")
            .field(
                "file",
                &self
                    .pending_open
                    .as_ref()
                    .map(|pending_open| &pending_open.file),
            )
            .field("marks", &self.sketch.borrow().marks().len())
            .field("unsaved", &self.is_unsaved())
            .finish_non_exhaustive()
    }
}
