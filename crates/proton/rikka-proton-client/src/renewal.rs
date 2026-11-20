use crate::session::Session;

#[derive(Debug)]
pub enum Renewal {
    Unchanged,
    Updated(Box<Session>),
    Rekeyed(Box<Session>),
}
