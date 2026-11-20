use std::ops::Deref;

use rikka_ui::{Handler, StateAccess};

pub struct Reach<S: ?Sized + 'static> {
    lookup: for<'a, 'b> fn(&'a mut StateAccess<'b>) -> Option<&'a mut S>,
}

impl<S: ?Sized + 'static> Reach<S> {
    pub fn new(lookup: for<'a, 'b> fn(&'a mut StateAccess<'b>) -> Option<&'a mut S>) -> Self {
        Self { lookup }
    }

    pub fn act<R: 'static>(self, act: impl Fn(&mut S) -> R + 'static) -> Handler<(), R> {
        let lookup = self.lookup;
        Handler::from_fn(move |access, ()| Some(act(lookup(access)?)))
    }

    pub fn act_with<A: 'static, R: 'static>(
        self,
        act: impl Fn(&mut S, A) -> R + 'static,
    ) -> Handler<(A,), R> {
        let lookup = self.lookup;
        Handler::from_fn(move |access, (value,)| Some(act(lookup(access)?, value)))
    }

    pub fn act_with_pair<A: 'static, B: 'static, R: 'static>(
        self,
        act: impl Fn(&mut S, A, B) -> R + 'static,
    ) -> Handler<(A, B), R> {
        let lookup = self.lookup;
        Handler::from_fn(move |access, (one, two)| Some(act(lookup(access)?, one, two)))
    }
}

impl<S: ?Sized + 'static> Clone for Reach<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: ?Sized + 'static> Copy for Reach<S> {}

impl<S: ?Sized + 'static> std::fmt::Debug for Reach<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reach").finish_non_exhaustive()
    }
}

pub struct Desk<'a, S: ?Sized + 'static> {
    held: &'a S,
    reach: Reach<S>,
}

impl<'a, S: ?Sized + 'static> Desk<'a, S> {
    pub fn new(held: &'a S, reach: Reach<S>) -> Self {
        Self { held, reach }
    }

    pub fn reach(self) -> Reach<S> {
        self.reach
    }

    pub fn act<R: 'static>(self, act: impl Fn(&mut S) -> R + 'static) -> Handler<(), R> {
        self.reach.act(act)
    }

    pub fn act_with<A: 'static, R: 'static>(
        self,
        act: impl Fn(&mut S, A) -> R + 'static,
    ) -> Handler<(A,), R> {
        self.reach.act_with(act)
    }

    pub fn act_with_pair<A: 'static, B: 'static, R: 'static>(
        self,
        act: impl Fn(&mut S, A, B) -> R + 'static,
    ) -> Handler<(A, B), R> {
        self.reach.act_with_pair(act)
    }
}

impl<S: ?Sized + 'static> Clone for Desk<'_, S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: ?Sized + 'static> Copy for Desk<'_, S> {}

impl<S: ?Sized + 'static> Deref for Desk<'_, S> {
    type Target = S;

    fn deref(&self) -> &S {
        self.held
    }
}

impl<S: ?Sized + 'static> std::fmt::Debug for Desk<'_, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Desk").finish_non_exhaustive()
    }
}
