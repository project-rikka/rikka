use rikka_sync::Mutex;

use rikka_gpu::{ForeignImage, Ticket};

pub struct Imported {
    pub(crate) image: ForeignImage,
    origin: (u32, u32),
    extent: (u32, u32),
    read: Mutex<Option<Ticket>>,
}

impl Imported {
    pub(crate) fn new(image: ForeignImage) -> Imported {
        let [width, height] = image.extent();
        Imported {
            image,
            origin: (0, 0),
            extent: (width, height),
            read: Mutex::new(None),
        }
    }

    #[must_use]
    pub fn cropped(self, (x, y): (u32, u32), (width, height): (u32, u32)) -> Imported {
        let [whole_width, whole_height] = self.image.extent();
        let origin = (x.min(whole_width - 1), y.min(whole_height - 1));
        let extent = (
            width.clamp(1, whole_width - origin.0),
            height.clamp(1, whole_height - origin.1),
        );
        Imported {
            origin,
            extent,
            ..self
        }
    }

    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.last_read().is_none_or(|ticket| ticket.is_ready())
    }

    pub fn wait_idle(&self) -> Result<(), rikka_gpu::Error> {
        match self.last_read() {
            Some(ticket) => ticket.wait(),
            None => Ok(()),
        }
    }

    #[must_use]
    pub fn extent(&self) -> (u32, u32) {
        self.extent
    }

    pub(crate) fn origin(&self) -> (u32, u32) {
        self.origin
    }

    fn last_read(&self) -> Option<Ticket> {
        self.read.lock().clone()
    }

    pub(crate) fn read_by(&self, ticket: Ticket) {
        *self.read.lock() = Some(ticket);
    }
}

impl std::fmt::Debug for Imported {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Imported")
            .field("origin", &self.origin)
            .field("extent", &self.extent)
            .finish_non_exhaustive()
    }
}
