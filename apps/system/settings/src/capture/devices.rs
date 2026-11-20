use rikka_audio::{AudioCapture, CaptureDevice, CaptureKind};
use rikka_ui::Label;

#[derive(Clone, Debug, Default)]
pub(crate) struct Devices(Vec<CaptureDevice>);

impl Devices {
    pub(crate) fn probe(&mut self) {
        self.0 = AudioCapture::devices().unwrap_or_default();
    }

    fn of_kind(&self, kind: Option<CaptureKind>) -> impl Iterator<Item = &CaptureDevice> {
        self.0
            .iter()
            .filter(move |device| kind == Some(device.kind))
    }

    pub(crate) fn labels(&self, kind: Option<CaptureKind>) -> Vec<Label> {
        self.of_kind(kind)
            .map(|device| Label::from(device.name.as_str()))
            .collect()
    }

    #[must_use]
    pub(crate) fn at(&self, kind: Option<CaptureKind>, named: &str) -> Option<usize> {
        match named.is_empty() {
            true => None,
            false => self.of_kind(kind).position(|it| it.name == named),
        }
    }

    #[must_use]
    pub(crate) fn named(&self, kind: Option<CaptureKind>, at: Option<usize>) -> String {
        at.and_then(|at| self.of_kind(kind).nth(at))
            .map(|device| device.name.clone())
            .unwrap_or_default()
    }
}
