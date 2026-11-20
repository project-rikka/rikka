use glam::Vec2;

use crate::composite::{
    Composite, CompositeInput, CompositeNode, Glow, Operation, RenderLayer, Tone,
};
use crate::draw::Frame;
use crate::effect::{BLOCK_ALIGNMENT, EffectPool, ResolvedEffect, Source, Wiring};
use crate::error::{Error, Result};
use crate::look::Look;
use crate::look::resolve::{SOLAR_ILLUMINANCE, luminance, sun_color};
use crate::post::{GpuGlow, GpuGrade, sun_bearing};
use rikka_gpu::texture::TexturePool;

const IMAGE: &[(&str, Wiring)] = &[(Composite::IMAGE, Wiring::Required)];
const GLOWING: &[(&str, Wiring)] = &[
    (Composite::IMAGE, Wiring::Required),
    (Glow::SOURCE, Wiring::Optional),
];
const BLOOM_MIX: f32 = 0.12;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Planned {
    pub reachable: bool,
    pub context: Option<usize>,
    pub reading: Option<usize>,
    pub block: u64,
    pub effect: Option<usize>,
    pub passthrough: Option<CompositeInput>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Reads {
    pub emission: bool,
    pub layers: bool,
    pub motion: bool,
    pub previous: bool,
}

#[derive(Default)]
pub(crate) struct CompositePlan {
    nodes: Vec<Planned>,
    effects: Vec<ResolvedEffect>,
    blocks: Vec<u8>,
    textures: Vec<u32>,
    reads: Reads,
    first_exposure: Option<usize>,
    tone: Option<Tone>,
}

struct Sockets<'a>(&'a [(&'static str, Wiring)]);

impl Sockets<'_> {
    fn has(&self, name: &str) -> bool {
        self.0.iter().any(|&(socket, _)| socket == name)
    }

    fn required(&self) -> impl Iterator<Item = &str> {
        self.0
            .iter()
            .filter(|&&(_, wiring)| wiring == Wiring::Required)
            .map(|&(socket, _)| socket)
    }

    fn primary(&self) -> Option<&'static str> {
        self.0.first().map(|&(socket, _)| socket)
    }
}

fn invalid(index: usize, message: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("composite node {index}: {message}"))
}

impl CompositePlan {
    pub(crate) fn nodes(&self) -> &[Planned] {
        &self.nodes
    }

    pub(crate) fn effect(&self, planned: &Planned) -> Option<&ResolvedEffect> {
        planned.effect.map(|index| &self.effects[index])
    }

    pub(crate) fn reads(&self) -> Reads {
        self.reads
    }

    pub(crate) fn first_exposure(&self) -> Option<usize> {
        self.first_exposure
    }

    pub(crate) fn tone(&self) -> Option<Tone> {
        self.tone
    }

    pub(crate) fn blocks(&self) -> &[u8] {
        &self.blocks
    }

    pub(crate) fn texture_slots(&self) -> impl Iterator<Item = u32> + '_ {
        self.textures.iter().copied()
    }

    fn clear(&mut self) {
        self.nodes.clear();
        self.effects.clear();
        self.blocks.clear();
        self.textures.clear();
        self.reads = Reads::default();
        self.first_exposure = None;
        self.tone = None;
    }

    pub(crate) fn resolve(
        &mut self,
        frame: &Frame,
        look: Option<&Look>,
        pool: &EffectPool,
        textures: &TexturePool,
    ) -> Result<()> {
        self.clear();
        let Some(look) = look else {
            return Ok(());
        };
        let composite = &look.composite;
        let count = composite.nodes.len();
        for (index, node) in composite.nodes.iter().enumerate() {
            let sockets = sockets(node, pool)?;
            validate(index, node, &composite.nodes[..index], &sockets)?;
            let context = match node.operation {
                Operation::Exposure(_) => Some(index),
                _ => self.primary_context(node, &sockets),
            };
            let reading = match node.operation {
                Operation::Exposure(_) => self.primary_context(node, &sockets),
                _ => context,
            };
            self.nodes.push(Planned {
                context,
                reading,
                ..Planned::default()
            });
        }
        match composite.output {
            CompositeInput::Node(index) if index >= count => {
                return Err(Error::Invalid(format!(
                    "composite output names node {index} of {count}"
                )));
            }
            CompositeInput::Node(index) => self.nodes[index].reachable = true,
            CompositeInput::Layer(layer) => self.read_layer(layer),
        }
        for index in (0..count).rev() {
            if !self.nodes[index].reachable {
                continue;
            }
            let node = &composite.nodes[index];
            self.reach(node, pool)?;
            self.plan_node(index, node, frame, look, pool, textures)?;
        }
        Ok(())
    }

    fn primary_context(&self, node: &CompositeNode, sockets: &Sockets<'_>) -> Option<usize> {
        let primary = node.input(sockets.primary()?)?;
        match primary {
            CompositeInput::Node(index) => self.nodes[index].context,
            CompositeInput::Layer(_) => None,
        }
    }

    fn read_layer(&mut self, layer: RenderLayer) {
        match layer {
            RenderLayer::Emission => self.reads.emission = true,
            RenderLayer::Layers => self.reads.layers = true,
            RenderLayer::Velocity => self.reads.motion = true,
            RenderLayer::Previous => self.reads.previous = true,
            RenderLayer::Image
            | RenderLayer::Depth
            | RenderLayer::Normals
            | RenderLayer::Occlusion => {}
        }
    }

    fn reach(&mut self, node: &CompositeNode, pool: &EffectPool) -> Result<()> {
        let used = |name: &str| match &node.operation {
            Operation::Glow(glow) => name == Composite::IMAGE || glow.glows(),
            _ => true,
        };
        let primary = match &node.operation {
            Operation::Effect(instance) if !instance.enabled => {
                pool.program(instance.effect)?.primary()
            }
            _ => None,
        };
        for (name, input) in &node.inputs {
            let used = match primary {
                Some(primary) => name == primary,
                None => used(name),
            };
            if !used {
                continue;
            }
            match *input {
                CompositeInput::Node(index) => self.nodes[index].reachable = true,
                CompositeInput::Layer(layer) => self.read_layer(layer),
            }
        }
        Ok(())
    }

    fn plan_node(
        &mut self,
        index: usize,
        node: &CompositeNode,
        frame: &Frame,
        look: &Look,
        pool: &EffectPool,
        textures: &TexturePool,
    ) -> Result<()> {
        match &node.operation {
            Operation::Exposure(_) => self.first_exposure = Some(index),
            Operation::Tonemap(tonemap) => self.tone = Some(tonemap.tone),
            Operation::MotionBlur(blur) => self.reads.motion |= blur.is_active(),
            Operation::Glow(glow) if glow.is_active() => {
                let additive = node.input(Glow::SOURCE).is_some();
                let block = glow_block(glow, additive, frame, look);
                self.nodes[index].block = self.push_block(bytemuck::bytes_of(&block));
            }
            Operation::Grade(grade) => {
                let block = GpuGrade::of(grade);
                self.nodes[index].block = self.push_block(bytemuck::bytes_of(&block));
            }
            Operation::Effect(instance) if instance.enabled => {
                let resolved =
                    pool.resolve(instance, textures, &mut self.blocks, &mut self.textures)?;
                let program = &resolved.program;
                self.reads.emission |= program.reads(Source::Emission);
                self.reads.layers |= program.reads(Source::Layers);
                self.reads.motion |= program.reads(Source::Velocity);
                self.nodes[index].effect = Some(self.effects.len());
                self.effects.push(resolved);
            }
            Operation::Effect(instance) => {
                let program = pool.program(instance.effect)?;
                self.nodes[index].passthrough =
                    program.primary().and_then(|socket| node.input(socket));
            }
            Operation::Glow(_) | Operation::DepthOfField(_) | Operation::Lens(_) => {}
        }
        Ok(())
    }

    fn push_block(&mut self, bytes: &[u8]) -> u64 {
        let start = self.blocks.len().next_multiple_of(BLOCK_ALIGNMENT);
        self.blocks.resize(start, 0);
        self.blocks.extend_from_slice(bytes);
        start as u64
    }
}

impl Composite {
    pub(crate) fn draws_emission(&self, pool: &EffectPool) -> bool {
        let emission = CompositeInput::Layer(RenderLayer::Emission);
        self.output == emission
            || self.nodes.iter().any(|node| {
                node.inputs.iter().any(|&(_, input)| input == emission)
                    || match &node.operation {
                        Operation::Effect(instance) => pool
                            .program(instance.effect)
                            .is_ok_and(|program| program.reads(Source::Emission)),
                        _ => false,
                    }
            })
    }
}

fn sockets<'a>(node: &CompositeNode, pool: &'a EffectPool) -> Result<Sockets<'a>> {
    Ok(match &node.operation {
        Operation::Glow(_) => Sockets(GLOWING),
        Operation::Effect(instance) => Sockets(&pool.program(instance.effect)?.inputs),
        Operation::Exposure(_)
        | Operation::MotionBlur(_)
        | Operation::DepthOfField(_)
        | Operation::Grade(_)
        | Operation::Tonemap(_)
        | Operation::Lens(_) => Sockets(IMAGE),
    })
}

fn validate(
    index: usize,
    node: &CompositeNode,
    earlier: &[CompositeNode],
    sockets: &Sockets<'_>,
) -> Result<()> {
    if earlier.iter().any(|other| other.key == node.key) {
        return Err(invalid(
            index,
            format_args!("key {} is used twice", node.key),
        ));
    }
    for (at, (name, input)) in node.inputs.iter().enumerate() {
        if !sockets.has(name) {
            return Err(invalid(index, format_args!("has no socket `{name}`")));
        }
        if node.inputs[..at].iter().any(|(known, _)| known == name) {
            return Err(invalid(
                index,
                format_args!("socket `{name}` is wired twice"),
            ));
        }
        if let CompositeInput::Node(source) = *input
            && source >= index
        {
            return Err(invalid(
                index,
                format_args!("socket `{name}` reads node {source}, which does not come before it"),
            ));
        }
    }
    for required in sockets.required() {
        if node.input(required).is_none() {
            return Err(invalid(
                index,
                format_args!("socket `{required}` is not wired"),
            ));
        }
    }
    if let Operation::Effect(instance) = &node.operation
        && !instance.enabled
        && sockets.primary().is_none()
    {
        return Err(invalid(
            index,
            "a disabled effect without inputs has nothing to pass",
        ));
    }
    Ok(())
}

fn glow_block(glow: &Glow, additive: bool, frame: &Frame, look: &Look) -> GpuGlow {
    let (sun_direction, sun_facing, sun_strength) = match look.environment.sky() {
        Some(sky) => {
            let (direction, facing) = sun_bearing(frame.camera.view, sky.to_sun());
            let strength = (luminance(sun_color(sky)) / SOLAR_ILLUMINANCE).clamp(0.0, 1.0);
            (direction, facing, strength)
        }
        None => (Vec2::ZERO, 0.0, 0.0),
    };
    GpuGlow {
        bloom_tint: glow.tint.to_array(),
        bloom_amount: match additive {
            true => glow.bloom.max(0.0),
            false => (glow.bloom * BLOOM_MIX).clamp(0.0, 1.0),
        },
        streak_tint: glow.streak_tint.to_array(),
        streak_amount: glow.streaks.max(0.0),
        leak_tint: glow.leak_tint.to_array(),
        leak_amount: glow.leaks.max(0.0),
        mist_amount: glow.diffusion.max(0.0),
        flare_amount: glow.flare.max(0.0),
        additive: u32::from(additive),
        sun_facing,
        sun_direction: sun_direction.to_array(),
        sun_strength,
        pad: 0.0,
    }
}
