use std::rc::Rc;

use rikka_ui::Handler;

use crate::abi::{Closure, ThisThread};
use crate::ffi;
use crate::window::Model;

pub(crate) type Callback<F> = Closure<F, ThisThread>;

pub(crate) fn action(callback: Callback<ffi::ActionFn>) -> Handler {
    Handler::new(move |model: &mut Model| callback.call(&mut model.context()))
}

pub(crate) fn shared_action(callback: Rc<Callback<ffi::ActionFn>>) -> Handler {
    Handler::new(move |model: &mut Model| callback.call(&mut model.context()))
}

pub(crate) fn flag(callback: Callback<ffi::BoolFn>) -> Handler<(bool,)> {
    Handler::new(move |model: &mut Model, value: bool| callback.call(&mut model.context(), value))
}

pub(crate) fn fixed_flag(callback: Rc<Callback<ffi::BoolFn>>, value: bool) -> Handler {
    Handler::new(move |model: &mut Model| callback.call(&mut model.context(), value))
}

pub(crate) fn number(callback: Callback<ffi::DoubleFn>) -> Handler<(f32,)> {
    Handler::new(move |model: &mut Model, value: f32| {
        callback.call(&mut model.context(), f64::from(value));
    })
}

pub(crate) fn index(callback: Callback<ffi::IndexFn>) -> Handler<(usize,)> {
    Handler::new(move |model: &mut Model, index: usize| callback.call(&mut model.context(), index))
}
