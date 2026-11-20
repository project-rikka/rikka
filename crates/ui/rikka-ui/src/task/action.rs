use crate::task::{Handler, IntoHandler};
use crate::view::StateAccess;

pub struct Action<Args = (), R = ()>(Option<Handler<Args, R>>);

impl<Args: 'static, R: 'static> Action<Args, R> {
    pub fn new<Marker>(handler: impl IntoHandler<Args, R, Marker>) -> Self {
        Self(Some(handler.into_handler()))
    }

    #[must_use]
    pub fn handler(&self) -> Option<&Handler<Args, R>> {
        self.0.as_ref()
    }

    #[must_use]
    pub fn is_set(&self) -> bool {
        self.0.is_some()
    }

    #[must_use]
    pub fn active(&self, enabled: bool) -> bool {
        enabled && self.is_set()
    }

    pub fn call(&self, reach: &mut StateAccess<'_>, args: Args) -> Option<R> {
        self.0.as_ref()?.call(reach, args)
    }
}

impl<Args, R> Default for Action<Args, R> {
    fn default() -> Self {
        Self(None)
    }
}

impl<Args, R> Clone for Action<Args, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Args, R> PartialEq for Action<Args, R> {
    fn eq(&self, other: &Self) -> bool {
        self.0.is_some() == other.0.is_some()
    }
}

impl<Args, R> std::fmt::Debug for Action<Args, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Action").field(&self.0.is_some()).finish()
    }
}
