pub(crate) fn sorted(keys: impl IntoIterator<Item = u32>) -> Vec<u32> {
    let mut keys: Vec<u32> = keys.into_iter().collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Tally {
    frames: Vec<u32>,
    counts: Vec<u32>,
}

impl Tally {
    pub(crate) fn of<'a>(sets: impl IntoIterator<Item = &'a [u32]>) -> Self {
        let mut all: Vec<u32> = sets.into_iter().flatten().copied().collect();
        all.sort_unstable();
        let mut tally = Self::default();
        for frame in all {
            match tally.frames.last() {
                Some(last) if *last == frame => {
                    if let Some(count) = tally.counts.last_mut() {
                        *count += 1;
                    }
                }
                _ => {
                    tally.frames.push(frame);
                    tally.counts.push(1);
                }
            }
        }
        tally
    }

    pub(crate) fn frames(&self) -> &[u32] {
        &self.frames
    }

    pub(crate) fn swapped(&self, gone: &[u32], come: &[u32]) -> Self {
        let mut tally = Self {
            frames: Vec::with_capacity(self.frames.len() + come.len()),
            counts: Vec::with_capacity(self.frames.len() + come.len()),
        };
        let mut gone = gone.iter().copied().peekable();
        let mut come = come.iter().copied().peekable();
        let mut held = self
            .frames
            .iter()
            .copied()
            .zip(self.counts.iter().copied())
            .peekable();
        loop {
            let frame = [
                held.peek().map(|(frame, _)| *frame),
                gone.peek().copied(),
                come.peek().copied(),
            ]
            .into_iter()
            .flatten()
            .min();
            let Some(frame) = frame else {
                break;
            };
            let mut count = held.next_if(|(at, _)| *at == frame).map_or(0, |(_, n)| n);
            if gone.next_if_eq(&frame).is_some() {
                count = count.saturating_sub(1);
            }
            if come.next_if_eq(&frame).is_some() {
                count += 1;
            }
            if count > 0 {
                tally.frames.push(frame);
                tally.counts.push(count);
            }
        }
        tally
    }
}
