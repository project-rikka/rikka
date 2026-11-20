use rikka_types::WindowIdentifier;
use rikka_ui::WindowRef;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowParent {
    Window(WindowRef),
    Foreign(WindowIdentifier),
}

impl From<WindowRef> for WindowParent {
    fn from(window: WindowRef) -> WindowParent {
        WindowParent::Window(window)
    }
}

impl From<WindowIdentifier> for WindowParent {
    fn from(identifier: WindowIdentifier) -> WindowParent {
        WindowParent::Foreign(identifier)
    }
}
