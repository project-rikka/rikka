use std::collections::VecDeque;

use rikka_types::Rect;

const MAX_AREAS: usize = 16;

const LOGGED: usize = 8;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Repaint {
    #[default]
    Clean,
    Within(Vec<Rect>),
    Whole,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Changes {
    pub repaint: Repaint,
    pub serial: u64,
}

impl Changes {
    pub const UNKNOWN: Changes = Changes {
        repaint: Repaint::Whole,
        serial: 0,
    };
}

impl Repaint {
    fn add(&mut self, damage: Option<&[Rect]>) {
        let Some(damage) = damage.filter(|areas| !areas.is_empty()) else {
            *self = Repaint::Whole;
            return;
        };
        match self {
            Repaint::Whole => {}
            Repaint::Clean => *self = Repaint::Within(damage.to_vec()),
            Repaint::Within(areas) => {
                areas.extend_from_slice(damage);
                if areas.len() > MAX_AREAS {
                    let bounds = areas
                        .iter()
                        .copied()
                        .reduce(|joined, area| joined.union(&area))
                        .unwrap_or(Rect::ZERO);
                    *areas = vec![bounds];
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct DamageLog {
    serial: u64,
    entries: VecDeque<(u64, Option<Vec<Rect>>)>,
}

impl DamageLog {
    pub fn record(&mut self, areas: Option<Vec<Rect>>) -> bool {
        self.serial += 1;
        let partial = areas.as_ref().is_some_and(|areas| !areas.is_empty());
        self.entries.push_back((self.serial, areas));
        while self.entries.len() > LOGGED {
            self.entries.pop_front();
        }
        partial
    }

    #[must_use]
    pub fn since(&self, since: u64) -> Changes {
        let serial = self.serial;
        if since == serial {
            return Changes {
                repaint: Repaint::Clean,
                serial,
            };
        }
        if since > serial {
            return Changes {
                repaint: Repaint::Whole,
                serial,
            };
        }
        let covered = self
            .entries
            .front()
            .is_some_and(|(first, _)| *first <= since.saturating_add(1));
        if !covered {
            return Changes {
                repaint: Repaint::Whole,
                serial,
            };
        }
        let mut repaint = Repaint::Clean;
        for (logged, areas) in &self.entries {
            if *logged > since {
                repaint.add(areas.as_deref());
            }
        }
        Changes { repaint, serial }
    }
}

pub(crate) fn merged(earlier: Option<Vec<Rect>>, later: Option<Vec<Rect>>) -> Option<Vec<Rect>> {
    let mut repaint = Repaint::Clean;
    repaint.add(earlier.as_deref());
    repaint.add(later.as_deref());
    match repaint {
        Repaint::Within(areas) => Some(areas),
        Repaint::Clean | Repaint::Whole => None,
    }
}
