use std::time::Duration;

use crate::source::SourceId;

struct Mark {
    at: u64,
    source: SourceId,
    start: Duration,
    rate: u32,
    end: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transition {
    Started(SourceId),
    Advanced { from: SourceId, to: SourceId },
}

#[derive(Default)]
pub(crate) struct Timeline {
    marks: Vec<Mark>,
    current_source: Option<SourceId>,
}

impl Timeline {
    pub(crate) fn clear(&mut self) {
        self.marks.clear();
        self.current_source = None;
    }

    pub(crate) fn restart(&mut self, source: SourceId, at: u64, start: Duration, rate: u32) {
        self.marks.clear();
        self.begin(source, at, start, rate);
    }

    pub(crate) fn begin(&mut self, source: SourceId, at: u64, start: Duration, rate: u32) {
        self.marks.retain(|mark| mark.at < at);
        self.marks.push(Mark {
            at,
            source,
            start,
            rate: rate.max(1),
            end: None,
        });
    }

    pub(crate) fn finish(&mut self, source: SourceId, at: u64) {
        if let Some(mark) = self
            .marks
            .iter_mut()
            .rev()
            .find(|mark| mark.source == source)
        {
            mark.end = Some(at);
        }
    }

    pub(crate) fn start_of(&self, source: SourceId) -> Option<u64> {
        self.marks
            .iter()
            .rev()
            .find(|mark| mark.source == source)
            .map(|mark| mark.at)
    }

    pub(crate) fn cancel_from(&mut self, at: u64) {
        self.marks.retain(|mark| mark.at < at);
        if let Some(mark) = self.marks.last_mut() {
            mark.end = mark.end.map(|end| end.min(at));
        }
    }

    pub(crate) fn end(&self) -> Option<u64> {
        self.marks.last()?.end
    }

    pub(crate) fn current_source(&self) -> Option<SourceId> {
        self.current_source
    }

    pub(crate) fn locate(&self, played: u64) -> Option<(SourceId, Duration)> {
        let mark = self.current(played)?;
        let reached = played.clamp(mark.at, mark.end.unwrap_or(u64::MAX).max(mark.at)) - mark.at;
        let offset = Duration::from_secs_f64(reached as f64 / f64::from(mark.rate));
        Some((mark.source, mark.start + offset))
    }

    pub(crate) fn cross(&mut self, played: u64) -> Option<Transition> {
        let index = self.current_index(played)?;
        self.marks.drain(..index);
        let source = self.marks.first()?.source;
        match self.current_source.replace(source) {
            None => Some(Transition::Started(source)),
            Some(from) if from != source => Some(Transition::Advanced { from, to: source }),
            Some(_) => None,
        }
    }

    fn current(&self, played: u64) -> Option<&Mark> {
        self.marks.get(self.current_index(played)?)
    }

    fn current_index(&self, played: u64) -> Option<usize> {
        if self.marks.is_empty() {
            return None;
        }
        Some(
            self.marks
                .iter()
                .rposition(|mark| mark.at <= played)
                .unwrap_or(0),
        )
    }
}
