#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Counting,
    Compressing,
    Receiving,
    Resolving,
    Writing,
    Updating,
}

impl Phase {
    fn from_title(title: &str) -> Option<Phase> {
        match title {
            "Enumerating objects" | "Counting objects" => Some(Phase::Counting),
            "Compressing objects" => Some(Phase::Compressing),
            "Receiving objects" | "Unpacking objects" => Some(Phase::Receiving),
            "Resolving deltas" => Some(Phase::Resolving),
            "Writing objects" => Some(Phase::Writing),
            "Updating files" | "Checking out files" => Some(Phase::Updating),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    phase: Phase,
    fraction: f32,
}

impl Progress {
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    #[must_use]
    pub fn fraction(&self) -> f32 {
        self.fraction
    }

    #[must_use]
    pub fn percent(&self) -> u8 {
        (self.fraction * 100.0).floor() as u8
    }
}

pub(crate) struct Stage {
    phase: Phase,
    weight: f32,
}

const fn stage(phase: Phase, weight: f32) -> Stage {
    Stage { phase, weight }
}

pub(crate) const FETCH: [Stage; 3] = [
    stage(Phase::Compressing, 0.1),
    stage(Phase::Receiving, 0.7),
    stage(Phase::Resolving, 0.2),
];

pub(crate) const PULL: [Stage; 4] = [
    stage(Phase::Compressing, 0.1),
    stage(Phase::Receiving, 0.6),
    stage(Phase::Resolving, 0.1),
    stage(Phase::Updating, 0.2),
];

pub(crate) const PUSH: [Stage; 3] = [
    stage(Phase::Compressing, 0.2),
    stage(Phase::Writing, 0.7),
    stage(Phase::Resolving, 0.1),
];

pub(crate) struct Meter<'a> {
    stages: &'a [Stage],
    shown: Option<Progress>,
}

impl<'a> Meter<'a> {
    pub(crate) fn new(stages: &'a [Stage]) -> Meter<'a> {
        Meter {
            stages,
            shown: None,
        }
    }

    pub(crate) fn read(&mut self, line: &str) -> Reading {
        let Some((phase, ratio)) = parse(line) else {
            return Reading::Other;
        };
        let reached = self.reached(phase, ratio);
        let fraction = self
            .shown
            .map_or(reached, |shown| shown.fraction.max(reached));
        let progress = Progress { phase, fraction };
        let moved = self
            .shown
            .is_none_or(|shown| shown.phase != phase || shown.percent() != progress.percent());
        self.shown = Some(progress);
        match moved {
            true => Reading::Moved(progress),
            false => Reading::Same,
        }
    }

    fn reached(&self, phase: Phase, ratio: f32) -> f32 {
        let Some(at) = self.stages.iter().position(|stage| stage.phase == phase) else {
            return self.shown.map_or(0.0, |shown| shown.fraction);
        };
        let done: f32 = self.stages[..at].iter().map(|stage| stage.weight).sum();
        (done + self.stages[at].weight * ratio).min(1.0)
    }
}

pub(crate) enum Reading {
    Moved(Progress),
    Same,
    Other,
}

pub(crate) fn is_phase(line: &str) -> bool {
    line.split_once(':')
        .is_some_and(|(title, _)| Phase::from_title(title.trim()).is_some())
}

fn parse(line: &str) -> Option<(Phase, f32)> {
    let line = line.strip_prefix("remote: ").unwrap_or(line);
    let (title, rest) = line.split_once(':')?;
    let phase = Phase::from_title(title.trim())?;
    let (percent, _) = rest.trim_start().split_once('%')?;
    let percent: u8 = percent.parse().ok()?;
    Some((phase, f32::from(percent.min(100)) / 100.0))
}
