use crate::decoding::backend::Block;

pub(crate) enum Cut {
    Kept,
    Skipped,
    Past,
}

pub(crate) struct Window {
    floor: i64,
    end: Option<i64>,
    channels: usize,
}

impl Window {
    pub(crate) fn new(end: Option<u64>, channels: u16) -> Window {
        Window {
            floor: 0,
            end: end.and_then(|end| i64::try_from(end).ok()),
            channels: usize::from(channels.max(1)),
        }
    }

    pub(crate) fn start_at(&mut self, frame: u64) {
        self.floor = i64::try_from(frame).unwrap_or(i64::MAX);
    }

    pub(crate) fn clamp(&self, frame: u64) -> u64 {
        self.end
            .and_then(|end| u64::try_from(end).ok())
            .map_or(frame, |end| frame.min(end))
    }

    pub(crate) fn cut(&self, block: &Block, out: &mut Vec<f32>) -> Cut {
        let first = block.start;
        if self.end.is_some_and(|end| first >= end) {
            return Cut::Past;
        }
        let last = first + (block.samples.len() / self.channels) as i64;
        let low = first.max(self.floor);
        let high = self.end.map_or(last, |end| last.min(end));
        if high <= low {
            return Cut::Skipped;
        }
        let from = (low - first) as usize * self.channels;
        let to = (high - first) as usize * self.channels;
        out.extend_from_slice(&block.samples[from..to]);
        Cut::Kept
    }
}
