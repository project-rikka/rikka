use crate::member::Member;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub default_layout: String,
    pub members: Vec<Member>,
}
