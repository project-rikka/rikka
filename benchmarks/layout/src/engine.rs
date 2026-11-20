use crate::spec::{Extent, Placed, Spec};

pub trait Engine: Sized {
    const NAME: &'static str;

    fn build(spec: &Spec) -> Self;

    fn compute(&mut self);

    fn resize(&mut self, width: f32);

    fn resize_leaf(&mut self, node: usize, content: Extent);

    fn placed(&self) -> Vec<Placed>;
}
