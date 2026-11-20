use crate::pending::Pending;
use crate::session::Session;

pub(crate) trait Page {
    fn opened(&mut self, _: &Pending, _: &Session) {}

    fn closed(&mut self) {}
}
