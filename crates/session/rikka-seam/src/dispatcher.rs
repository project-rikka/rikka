use crate::call::Call;

pub trait Dispatcher: Send + Sync + 'static {
    fn enqueue(&self, call: Call);
}
