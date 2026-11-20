use animate_hold::{Hold, HoldId};

#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub id: HoldId,
    pub hold: Hold,
    pub holding: bool,
    pub owner: String,
}
