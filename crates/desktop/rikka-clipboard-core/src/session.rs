use crate::error::Result;
use crate::offer::Offer;
use crate::selection::Selection;

pub trait Session: Send + Sync {
    fn mime_types(&self, selection: Selection) -> Result<Vec<String>>;

    fn read(&self, selection: Selection, mime: &str) -> Result<Vec<u8>>;

    fn publish(&self, selection: Selection, offer: Offer) -> Result<()>;

    fn watch(&self, changed: Box<dyn Fn() + Send>) -> Result<()>;

    fn close(&self);
}
