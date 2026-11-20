type Notice = Box<dyn FnOnce() + Send>;

#[must_use]
#[derive(Default)]
pub struct Lease {
    released: Option<Notice>,
}

impl Lease {
    pub fn new() -> Lease {
        Lease::default()
    }

    pub fn on_released(mut self, released: impl FnOnce() + Send + 'static) -> Lease {
        self.released = Some(Box::new(released));
        self
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(released) = self.released.take() {
            released();
        }
    }
}

impl std::fmt::Debug for Lease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Lease")
            .field("releasing", &self.released.is_some())
            .finish()
    }
}
