use crate::action::Action;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Request {
    Ask(Action),
    Brighten(f32),
    Reread,
}
