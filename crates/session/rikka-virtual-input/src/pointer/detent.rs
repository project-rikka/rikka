pub(super) const NOTCH: i32 = 120;

#[derive(Default)]
pub(super) struct Detent(i32);

impl Detent {
    pub(super) fn turn(&mut self, value120: i32) -> i32 {
        if self.0.signum() != value120.signum() {
            self.0 = 0;
        }
        self.0 = self.0.saturating_add(value120);
        let notches = self.0 / NOTCH;
        self.0 -= notches * NOTCH;
        notches
    }
}
