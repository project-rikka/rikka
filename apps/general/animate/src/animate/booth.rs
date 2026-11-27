use std::sync::Arc;

use animate_booth::{Booth, Order, Performer, Preview, Sitter, Ticket};
use animate_cast::Figure;
use animate_library::{Entry, FigureKey, Shelf, Thumbnails, Variant};
use animate_library_browser::{Painter, Painting, Picture};
use rikka_picture::Feed;
use rikka_sync::Mutex;
use rikka_types::PixelSize;

const PREVIEW_SIDE: u32 = 384;

#[derive(Debug)]
pub(crate) struct BoothPainter {
    booth: Booth,
    performer: Mutex<Option<(FigureKey, Performer)>>,
}

#[derive(Debug)]
pub(crate) struct Previewing {
    feed: Feed,
    _preview: Preview,
}

impl Previewing {
    pub(crate) fn feed(&self) -> Feed {
        self.feed.clone()
    }
}

impl BoothPainter {
    pub(crate) fn spawn() -> Result<Self, String> {
        let booth = Booth::new().map_err(|error| error.to_string())?;
        Ok(Self {
            booth,
            performer: Mutex::new(None),
        })
    }

    pub(crate) fn perform(&self, key: FigureKey, figure: &Figure) {
        let mut performer = self.performer.lock();
        if performer
            .as_ref()
            .is_some_and(|(current, _)| *current == key)
        {
            return;
        }
        *performer = Some((
            key,
            Performer::Figure {
                key: key.get(),
                model: Arc::clone(&figure.model),
                body: figure.body.clone(),
                faces: Arc::clone(&figure.faces),
            },
        ));
    }

    pub(crate) fn preview(&self, entry: &Entry, variant: Variant) -> Option<Previewing> {
        let sitter = self.sitter(entry, variant)?;
        let feed = Feed::new();
        let order = Order {
            sitter,
            size: PixelSize::new(PREVIEW_SIDE, PREVIEW_SIDE),
        };
        let preview = self.booth.preview(order, feed.clone());
        Some(Previewing {
            feed,
            _preview: preview,
        })
    }

    fn sitter(&self, entry: &Entry, variant: Variant) -> Option<Sitter> {
        let path = entry.path().to_path_buf();
        Some(match entry.shelf() {
            Shelf::Models => Sitter::Model(path),
            Shelf::Stages => Sitter::Stage(path),
            Shelf::Accessories => Sitter::Accessory(path),
            Shelf::Cameras => Sitter::Camera(path),
            Shelf::Motions => {
                let performer = match variant {
                    Variant::Performed(key) => self
                        .performer
                        .lock()
                        .as_ref()
                        .filter(|(current, _)| *current == key)
                        .map(|(_, performer)| performer.clone())?,
                    Variant::Still => Performer::Mannequin,
                };
                Sitter::Motion {
                    motion: path,
                    performer,
                }
            }
            Shelf::Music => return None,
        })
    }
}

impl Painter for BoothPainter {
    fn paint(&self, entry: Arc<Entry>, variant: Variant) -> Painting {
        let Some(sitter) = self.sitter(&entry, variant) else {
            return Box::pin(async { Err("nothing to draw".to_owned()) });
        };
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let order = Order {
            sitter,
            size: PixelSize::new(Thumbnails::SIDE, Thumbnails::SIDE),
        };
        let ticket: Ticket = self.booth.render(order, move |painted| {
            let _ = sender.send(painted);
        });
        Box::pin(async move {
            let painted = receiver.await.map_err(|error| error.to_string())?;
            drop(ticket);
            let picture = painted.map_err(|error| error.to_string())?;
            Ok(Picture {
                width: picture.width,
                height: picture.height,
                rgba: picture.rgba,
            })
        })
    }
}
