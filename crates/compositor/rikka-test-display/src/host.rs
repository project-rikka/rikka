use rikka_compositor_server::DisplayHandle;

pub trait Host: Sized + 'static {
    fn display_handle(&self) -> &DisplayHandle<Self>;
}
