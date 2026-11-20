use std::fmt;

use crate::grid::{Grid, Motion};

pub const LIST_ROWS: usize = 10_000;

pub const CELL_COUNTS: [usize; 3] = [100, 1000, 3000];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    Cells { count: usize, motion: Motion },
    Scroll,
    Hover,
    Startup,
}

#[derive(Debug)]
pub struct ScenarioError(String);

impl fmt::Display for ScenarioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unknown scenario {:?}: expected cells-<count>-<anim|one|idle>, scroll, hover or startup",
            self.0
        )
    }
}

impl std::error::Error for ScenarioError {}

impl Scenario {
    #[must_use]
    pub fn all() -> Vec<Scenario> {
        let mut all = vec![Scenario::Startup];
        for count in CELL_COUNTS {
            for motion in Motion::ALL {
                all.push(Scenario::Cells { count, motion });
            }
        }
        all.push(Scenario::Scroll);
        all.push(Scenario::Hover);
        all
    }

    pub fn from_args() -> Result<Scenario, ScenarioError> {
        let name = std::env::args().nth(1).unwrap_or_default();
        name.parse()
    }

    #[must_use]
    pub fn grid(self) -> Option<Grid> {
        match self {
            Scenario::Cells { count, motion } => Some(Grid::new(count, motion)),
            Scenario::Scroll | Scenario::Hover | Scenario::Startup => None,
        }
    }

    #[must_use]
    pub fn scrolls(self) -> bool {
        self == Scenario::Scroll
    }

    #[must_use]
    pub fn animates(self) -> bool {
        match self {
            Scenario::Cells { motion, .. } => motion.animates(),
            Scenario::Scroll => true,
            Scenario::Hover | Scenario::Startup => false,
        }
    }
}

impl std::str::FromStr for Scenario {
    type Err = ScenarioError;

    fn from_str(name: &str) -> Result<Scenario, ScenarioError> {
        let refuse = || ScenarioError(name.to_owned());
        match name {
            "scroll" => Ok(Scenario::Scroll),
            "hover" => Ok(Scenario::Hover),
            "startup" => Ok(Scenario::Startup),
            _ => {
                let mut parts = name.split('-');
                let (Some("cells"), Some(count), Some(motion), None) =
                    (parts.next(), parts.next(), parts.next(), parts.next())
                else {
                    return Err(refuse());
                };
                Ok(Scenario::Cells {
                    count: count.parse().map_err(|_| refuse())?,
                    motion: Motion::from_name(motion).ok_or_else(refuse)?,
                })
            }
        }
    }
}

impl fmt::Display for Scenario {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scenario::Cells { count, motion } => {
                write!(formatter, "cells-{count}-{}", motion.name())
            }
            Scenario::Scroll => formatter.write_str("scroll"),
            Scenario::Hover => formatter.write_str("hover"),
            Scenario::Startup => formatter.write_str("startup"),
        }
    }
}
