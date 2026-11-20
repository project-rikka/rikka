mod declare;
mod layers;
mod operations;
mod plan;

pub use operations::{
    AgxLook, DepthOfField, Exposure, Glow, Grade, Lens, Metering, MotionBlur, Tone, ToneCurve,
    Tonemap,
};

pub(crate) use declare::{NodeOutput, declare};
pub(crate) use layers::{LayerImages, keep_output};
pub(crate) use plan::CompositePlan;

use crate::effect::EffectInstance;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenderLayer {
    Image,
    Depth,
    Normals,
    Velocity,
    Layers,
    Emission,
    Occlusion,
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompositeInput {
    Layer(RenderLayer),
    Node(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    Exposure(Exposure),
    MotionBlur(MotionBlur),
    DepthOfField(DepthOfField),
    Glow(Glow),
    Grade(Grade),
    Tonemap(Tonemap),
    Lens(Lens),
    Effect(EffectInstance),
}

macro_rules! operations {
    ($($variant:ident),*) => {
        $(impl From<$variant> for Operation {
            fn from(operation: $variant) -> Self {
                Self::$variant(operation)
            }
        })*
    };
}

operations!(
    Exposure,
    MotionBlur,
    DepthOfField,
    Glow,
    Grade,
    Tonemap,
    Lens
);

impl From<EffectInstance> for Operation {
    fn from(instance: EffectInstance) -> Self {
        Self::Effect(instance)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompositeNode {
    pub key: u64,
    pub operation: Operation,
    pub inputs: Vec<(String, CompositeInput)>,
}

impl CompositeNode {
    pub fn input(&self, socket: &str) -> Option<CompositeInput> {
        self.inputs
            .iter()
            .find(|(name, _)| name == socket)
            .map(|&(_, input)| input)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Composite {
    pub nodes: Vec<CompositeNode>,
    pub output: CompositeInput,
}

impl Default for Composite {
    fn default() -> Self {
        Self::standard()
    }
}

impl Composite {
    pub const IMAGE: &'static str = "image";

    pub fn new() -> Self {
        Self::passthrough()
    }

    pub fn passthrough() -> Self {
        Self {
            nodes: Vec::new(),
            output: CompositeInput::Layer(RenderLayer::Image),
        }
    }

    pub fn standard() -> Self {
        let mut composite = Self::new();
        composite.push(Exposure::default());
        composite.push(MotionBlur::default());
        composite.push(DepthOfField::default());
        composite.push(Glow::default());
        composite.push(Grade::default());
        composite.push(Tonemap::default());
        composite.push(Lens::default());
        composite
    }

    pub fn push(&mut self, operation: impl Into<Operation>) -> usize {
        let image = match self.nodes.len() {
            0 => CompositeInput::Layer(RenderLayer::Image),
            count => CompositeInput::Node(count - 1),
        };
        let index = self.node(operation, [(Self::IMAGE, image)]);
        self.output = CompositeInput::Node(index);
        index
    }

    pub fn node<S: Into<String>>(
        &mut self,
        operation: impl Into<Operation>,
        inputs: impl IntoIterator<Item = (S, CompositeInput)>,
    ) -> usize {
        let key = self
            .nodes
            .iter()
            .map(|node| node.key.wrapping_add(1))
            .max()
            .unwrap_or(0);
        self.nodes.push(CompositeNode {
            key,
            operation: operation.into(),
            inputs: inputs
                .into_iter()
                .map(|(name, input)| (name.into(), input))
                .collect(),
        });
        self.nodes.len() - 1
    }

    pub fn output(&mut self, output: CompositeInput) -> &mut Self {
        self.output = output;
        self
    }
}
