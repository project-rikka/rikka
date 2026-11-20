use crate::notification::Notification;

#[derive(Debug)]
pub enum Posted {
    Shown(Box<Notification>),
    Withdrawn(u32),
}
