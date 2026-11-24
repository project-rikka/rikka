use std::sync::Arc;

use crate::abi::violation;
use crate::ffi;
use crate::values::{color, finite};

#[derive(Debug)]
pub(crate) struct Gradient {
    linear: rikka_ui::Gradient,
}

impl Gradient {
    pub(crate) fn get(&self) -> rikka_ui::Gradient {
        self.linear
    }
}

pub(crate) fn gradient_new_linear(angle: f32, from: ffi::Color, to: ffi::Color) -> Arc<Gradient> {
    let degrees = finite(f64::from(angle), "gradient angle") as f32;
    Arc::new(Gradient {
        linear: rikka_ui::Gradient::linear(degrees.to_radians(), color(from), color(to)),
    })
}

pub(crate) fn gradient_with_stop(
    gradient: &Gradient,
    offset: f32,
    stop: ffi::Color,
) -> Arc<Gradient> {
    if !(0.0..=1.0).contains(&offset) {
        violation(format!("the stop offset {offset} is not between 0 and 1"));
    }
    match gradient.linear.with_stop(offset, color(stop)) {
        Ok(linear) => Arc::new(Gradient { linear }),
        Err(full) => violation(full.to_string()),
    }
}
