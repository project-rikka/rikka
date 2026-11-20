use crate::display::DisplayHandle;

pub trait Module<S: 'static> {
    fn install(self, display: &DisplayHandle<S>);
}
