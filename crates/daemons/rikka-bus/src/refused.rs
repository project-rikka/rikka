use crate::monitor::Apply;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused<A> {
    pub action: A,
    pub reason: String,
}

pub trait Refusing {
    type Action: PartialEq;

    fn refused(&mut self) -> &mut Option<Refused<Self::Action>>;

    fn answer(&mut self, action: Self::Action, error: Option<String>) -> bool {
        let refused = self.refused();
        match error {
            Some(reason) => refused.apply(Some(Refused { action, reason })),
            None => refused.take_if(|held| held.action == action).is_some(),
        }
    }
}

impl<A> std::fmt::Display for Refused<A> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.reason)
    }
}
