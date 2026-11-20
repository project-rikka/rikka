use crate::error::Result;
use crate::io::Endpoint;

#[derive(Debug)]
pub(crate) struct Attached<D: ?Sized> {
    device_name: Option<String>,
    device: Option<Box<D>>,
    failed_at: Option<u64>,
    lost: bool,
}

impl<D: Endpoint + ?Sized> Attached<D> {
    pub(crate) fn new(device_name: Option<String>) -> Attached<D> {
        Attached {
            device_name,
            device: None,
            failed_at: None,
            lost: false,
        }
    }

    pub(crate) fn device_name(&self) -> Option<&str> {
        self.device_name.as_deref()
    }

    pub(crate) fn attach(&mut self, device: Box<D>) {
        self.device = Some(device);
        self.failed_at = None;
        self.lost = false;
    }

    pub(crate) fn choose(&mut self, device_name: Option<String>) {
        if device_name != self.device_name {
            self.device_name = device_name;
            self.device = None;
            self.failed_at = None;
        }
    }

    pub(crate) fn poll(
        &mut self,
        generation: u64,
        open: impl Fn(Option<&str>) -> Result<Box<D>>,
    ) -> bool {
        if self.device.as_ref().is_some_and(|device| device.is_lost()) {
            return self.fail(generation);
        }
        if self.device.is_some() || self.failed_at == Some(generation) {
            return false;
        }
        let device = open(self.device_name.as_deref()).or_else(|error| match &self.device_name {
            Some(name) => {
                tracing::warn!(%error, name, "failed to open audio device, using default");
                open(None)
            }
            None => Err(error),
        });
        match device {
            Ok(device) => {
                self.attach(device);
                false
            }
            Err(error) => {
                tracing::warn!(%error, "failed to open audio device");
                self.fail(generation)
            }
        }
    }

    pub(crate) fn fail(&mut self, generation: u64) -> bool {
        self.device = None;
        self.failed_at = Some(generation);
        let newly = !self.lost;
        self.lost = true;
        newly
    }

    pub(crate) fn device(&mut self) -> Option<&mut D> {
        self.device.as_deref_mut()
    }

    pub(crate) fn release(&mut self) {
        self.device = None;
        self.failed_at = None;
    }
}
