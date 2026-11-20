use std::marker::PhantomData;
use std::rc::Rc;

use crate::task::effects::Acts;
use crate::view::StateAccess;

type Call<Args, R> = dyn Fn(&mut StateAccess<'_>, Args) -> Option<R>;

pub struct Handler<Args = (), R = ()> {
    call: Rc<Call<Args, R>>,
    acts: Acts,
}

impl<Args, R> Clone for Handler<Args, R> {
    fn clone(&self) -> Self {
        Self {
            call: Rc::clone(&self.call),
            acts: self.acts,
        }
    }
}

impl<Args, R> std::fmt::Debug for Handler<Args, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handler")
            .field("acts", &self.acts)
            .finish_non_exhaustive()
    }
}

impl<Args: 'static, R: 'static> Handler<Args, R> {
    pub fn new<M>(handler: impl IntoHandler<Args, R, M>) -> Self {
        handler.into_handler()
    }

    pub fn from_fn(call: impl Fn(&mut StateAccess<'_>, Args) -> Option<R> + 'static) -> Self {
        Self::acting(Acts::Plainly, call)
    }

    pub fn relaying(call: impl Fn(&mut StateAccess<'_>, Args) -> Option<R> + 'static) -> Self {
        Self::acting(Acts::Relaying, call)
    }

    fn targeted(call: impl Fn(&mut StateAccess<'_>, Args) -> Option<R> + 'static) -> Self {
        Self::acting(Acts::Tracked, call)
    }

    fn acting(
        acts: Acts,
        call: impl Fn(&mut StateAccess<'_>, Args) -> Option<R> + 'static,
    ) -> Self {
        Self {
            call: Rc::new(call),
            acts,
        }
    }

    pub fn call(&self, reach: &mut StateAccess<'_>, args: Args) -> Option<R> {
        crate::task::acting(reach.chain(), self.acts, || (self.call)(reach, args))
    }

    #[must_use]
    pub fn map<A: 'static>(self, adapt: impl Fn(A) -> Args + 'static) -> Handler<A, R> {
        let acts = self.acts;
        Handler {
            call: Rc::new(move |reach, args| (self.call)(reach, adapt(args))),
            acts,
        }
    }
}

impl<Args: 'static> Handler<Args, ()> {
    #[must_use]
    pub fn filter_map<A: 'static>(
        self,
        adapt: impl Fn(A) -> Option<Args> + 'static,
    ) -> Handler<A, bool> {
        Handler::relaying(move |reach, args| match adapt(args) {
            Some(args) => self.call(reach, args).map(|()| true),
            None => Some(false),
        })
    }
}

pub trait IntoHandler<Args, R, Marker>: 'static {
    fn into_handler(self) -> Handler<Args, R>;
}

impl<Args: 'static, R: 'static> IntoHandler<Args, R, ()> for Handler<Args, R> {
    fn into_handler(self) -> Handler<Args, R> {
        self
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct Plain<Args>(PhantomData<fn(Args)>);

#[doc(hidden)]
#[derive(Debug)]
pub struct Targeted<T, Args>(PhantomData<fn(T, Args)>);

macro_rules! handlers {
    ($(($($arg:ident $value:ident),*)),+ $(,)?) => {
        $(
            impl<$($arg: 'static,)* R: 'static, F> IntoHandler<($($arg,)*), R, Plain<($($arg,)*)>> for F
            where
                F: Fn($($arg),*) -> R + 'static,
            {
                fn into_handler(self) -> Handler<($($arg,)*), R> {
                    Handler::from_fn(move |_reach, ($($value,)*)| Some(self($($value),*)))
                }
            }

            impl<T: 'static, $($arg: 'static,)* R: 'static, F> IntoHandler<($($arg,)*), R, Targeted<T, ($($arg,)*)>> for F
            where
                F: Fn(&mut T, $($arg),*) -> R + 'static,
            {
                fn into_handler(self) -> Handler<($($arg,)*), R> {
                    Handler::targeted(move |reach, ($($value,)*)| Some(self(reach.get::<T>()?, $($value),*)))
                }
            }
        )+
    };
}

handlers!((), (A a), (A a, B b), (A a, B b, C c));
