use rikka_keyframe::Frame;

const WITHIN_SECOND: [u32; 4] = [1, 2, 5, 10];
const SECONDS: [u32; 6] = [1, 2, 5, 10, 15, 30];
const MINUTES: [u32; 7] = [1, 2, 5, 10, 15, 30, 60];
const TICK_ROOM: f64 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticks {
    pub major: u32,
    pub minor: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tick<F> {
    pub frame: F,
    pub major: bool,
}

impl Ticks {
    #[must_use]
    pub fn new(per_frame: f64, frames_per_second: u32, label_room: f32) -> Ticks {
        let second = frames_per_second.max(1);
        let steps: Vec<u32> = WITHIN_SECOND
            .into_iter()
            .filter(|step| *step < second)
            .chain(SECONDS.map(|step| step * second))
            .chain(MINUTES.map(|step| step * 60 * second))
            .collect();
        let largest = steps[steps.len() - 1];
        let major = steps
            .iter()
            .copied()
            .find(|step| f64::from(*step) * per_frame >= f64::from(label_room))
            .unwrap_or(largest);
        let minor = steps
            .iter()
            .copied()
            .find(|step| major.is_multiple_of(*step) && f64::from(*step) * per_frame >= TICK_ROOM)
            .unwrap_or(major);
        Ticks { major, minor }
    }

    pub fn across<F: Frame>(self, from: f64, to: f64) -> impl Iterator<Item = Tick<F>> {
        let minor = u64::from(self.minor);
        let major = u64::from(self.major);
        let first = (from.max(0.0) / minor as f64).floor() as u64 * minor;
        let last = to.max(0.0).ceil() as u64;
        (first..=last)
            .step_by(minor as usize)
            .map(move |frame| Tick {
                frame: F::from_f64(frame as f64),
                major: frame.is_multiple_of(major),
            })
    }
}
