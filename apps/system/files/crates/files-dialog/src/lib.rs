mod answer;
mod ask;
mod choice;
mod filter;
mod wire;

pub use answer::Answer;
pub use ask::{Ask, Kind};
pub use choice::{Choice, ChoiceOption, OFF, ON};
pub use filter::{Filter, Rule};

pub use rikka_portal::{Error, Reply, Result};

pub type Outcome = rikka_portal::Outcome<Answer>;
