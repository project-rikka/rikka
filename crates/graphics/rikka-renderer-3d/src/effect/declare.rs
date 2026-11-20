use ash::vk;
use smallvec::SmallVec;

use crate::composite::LayerImages;
use crate::effect::desc::{EffectUniform, TargetLifetime};
use crate::effect::pool::ResolvedEffect;
use crate::effect::program::{Field, Shader, Sink, Source, Step};
use crate::error::Result;
use crate::gpu::NONE_INDEX;
use crate::graph::{Attachment, FrameGraph, ImageId, ImageSpec, Load, PassScope, Size};
use crate::record::Record;
use crate::record::stage::{SceneImages, Stage};
use crate::viewport::COLOR_FORMAT;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::resource_table::ResourceTable;

const PUSH_BYTES: usize = 128;
const IMAGE: &str = "rikka.three.effect.image";
const HISTORY: &str = "rikka.three.effect.history";
const KEPT: &str = "rikka.three.effect.kept";
const KEPT_SHIFT: u32 = 56;

pub(crate) type Sockets = SmallVec<[Option<ImageId>; 4]>;

pub(crate) struct EffectNode<'f> {
    pub effect: &'f ResolvedEffect,
    pub key: u64,
    pub inputs: Sockets,
    pub emission: Option<ImageId>,
}

#[derive(Clone, Copy, Debug)]
enum Value {
    Sampled(ImageId),
    Fallback(ImageId, ImageId),
    Storage(ImageId),
    Size(ImageId),
    Texel(ImageId),
    Time(f32),
    Frame,
    Parameters(u64),
    Missing,
}

pub(crate) fn declare_effect<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    scene: &SceneImages,
    layers: &mut LayerImages,
    node: EffectNode<'f>,
) -> ImageId {
    let effect = node.effect;
    let program = &*effect.program;
    let normals = (program.reads(Source::Normals) || program.reads(Source::Velocity))
        .then(|| layers.surface(graph, scene));
    let depth = program
        .reads(Source::Depth)
        .then(|| layers.depth(graph, scene));
    let identity = program
        .reads(Source::Layers)
        .then(|| layers.identity(graph, scene));
    let kept_key = |index: usize| node.key ^ ((index as u64 + 1) << KEPT_SHIFT);
    let targets: SmallVec<[ImageId; 8]> = program
        .targets
        .iter()
        .enumerate()
        .map(|(index, target)| {
            let size = Size::Divided(target.divisor);
            graph.create(match target.lifetime {
                TargetLifetime::Frame => {
                    ImageSpec::new("rikka.three.effect.target", target.format, size)
                }
                TargetLifetime::Kept => {
                    ImageSpec::new(KEPT, target.format, size).current(kept_key(index))
                }
            })
        })
        .collect();
    let kept: SmallVec<[Option<ImageId>; 8]> = program
        .targets
        .iter()
        .enumerate()
        .map(|(index, target)| {
            let spec = ImageSpec::new(KEPT, target.format, Size::Divided(target.divisor));
            (target.lifetime == TargetLifetime::Kept)
                .then(|| graph.create(spec.previous(kept_key(index))))
        })
        .collect();
    let passed = node.inputs.first().copied().flatten();
    let spec = ImageSpec::new(HISTORY, COLOR_FORMAT, Size::Viewport);
    let output = match program.reads(Source::History) {
        true => graph.create(spec.current(node.key)),
        false => graph.create(ImageSpec::new(IMAGE, COLOR_FORMAT, Size::Viewport)),
    };
    let previous = program
        .reads(Source::History)
        .then(|| graph.create(spec.previous(node.key)));
    let frame_time = stage.frame.time;
    for step in &program.steps {
        let source = |source: Source| -> Option<Value> {
            Some(match source {
                Source::Input(index) => Value::Sampled(node.inputs[index]?),
                Source::Depth => Value::Sampled(depth.flatten()?),
                Source::Layers => Value::Sampled(identity.flatten()?),
                Source::Normals => Value::Sampled(normals.flatten()?.normals),
                Source::Velocity => Value::Sampled(normals.flatten()?.velocity),
                Source::Emission => Value::Sampled(node.emission?),
                Source::History => Value::Fallback(previous?, passed?),
                Source::Target(index) => Value::Sampled(targets[index]),
                Source::Kept(index) => Value::Fallback(kept[index]?, passed?),
            })
        };
        let sinks: SmallVec<[ImageId; 2]> = step
            .sinks
            .iter()
            .map(|sink| match *sink {
                Sink::Target(index) => targets[index],
                Sink::Image => output,
            })
            .collect();
        let primary = sinks[0];
        let mut sink_at = 0;
        let values: SmallVec<[(u32, Value); 12]> = step
            .fields
            .iter()
            .map(|&(offset, field)| {
                let value = match field {
                    Field::Read(read) => source(read).unwrap_or(Value::Missing),
                    Field::Write(_) => {
                        let image = sinks[sink_at];
                        sink_at += 1;
                        Value::Storage(image)
                    }
                    Field::Uniform(EffectUniform::Size) => Value::Size(primary),
                    Field::Uniform(EffectUniform::Texel) => Value::Texel(primary),
                    Field::Uniform(EffectUniform::Time) => Value::Time(frame_time),
                    Field::Uniform(EffectUniform::Frame) => Value::Frame,
                    Field::Parameters => Value::Parameters(effect.parameters),
                };
                (offset, value)
            })
            .collect();
        let reads: SmallVec<[ImageId; 8]> = values
            .iter()
            .flat_map(|(_, value)| match *value {
                Value::Sampled(image) => [Some(image), None],
                Value::Fallback(previous, input) => [Some(previous), Some(input)],
                _ => [None, None],
            })
            .flatten()
            .collect();
        declare_step(graph, step, reads, sinks, values);
    }
    output
}

fn declare_step<'f>(
    graph: &mut FrameGraph<'f>,
    step: &'f Step,
    reads: SmallVec<[ImageId; 8]>,
    sinks: SmallVec<[ImageId; 2]>,
    values: SmallVec<[(u32, Value); 12]>,
) {
    let primary = sinks[0];
    let record = move |record: &mut Record<'_>,
                       ctx: &mut GpuFrameCtx<'_>,
                       scope: &PassScope<'_>| {
        let push = assemble(&values, scope, ctx.resources)?;
        match &step.shader {
            Shader::Kernel(shader) => {
                let pipeline = record.mesh.pipelines.material_kernel(shader)?;
                let extent = scope.extent(primary);
                record.dispatch_pipeline(ctx, pipeline, &push, groups_for(extent, step.local));
                Ok(())
            }
            Shader::Fragment(shader) => record.effect_fullscreen(ctx, shader, step.format, &push),
        }
    };
    match step.shader {
        Shader::Kernel(_) => {
            let mut pass = graph.compute(step.label).read_all(reads.iter().copied());
            for &sink in &sinks {
                pass = pass.write(sink);
            }
            pass.record(record);
        }
        Shader::Fragment(_) => {
            graph
                .render(step.label)
                .color(Attachment::new(sinks[0], Load::Discard))
                .read_all(reads.iter().copied())
                .record(record);
        }
    }
}

fn groups_for(extent: vk::Extent2D, local: [u32; 2]) -> (u32, u32) {
    (
        extent.width.div_ceil(local[0]),
        extent.height.div_ceil(local[1]),
    )
}

fn assemble(
    values: &[(u32, Value)],
    scope: &PassScope<'_>,
    resources: &mut ResourceTable,
) -> Result<[u8; PUSH_BYTES]> {
    let mut push = [0u8; PUSH_BYTES];
    let mut put = |offset: u32, words: &[u32]| {
        let at = offset as usize;
        let bytes: &[u8] = bytemuck::cast_slice(words);
        push[at..at + bytes.len()].copy_from_slice(bytes);
    };
    for &(offset, value) in values {
        match value {
            Value::Sampled(image) => put(offset, &[sampled_slot(scope, resources, image)?]),
            Value::Fallback(previous, input) => {
                let image = match scope.fresh(previous) {
                    true => input,
                    false => previous,
                };
                put(offset, &[sampled_slot(scope, resources, image)?]);
            }
            Value::Storage(image) => put(offset, &[scope.storage(image)]),
            Value::Size(image) => {
                let extent = scope.extent(image);
                put(offset, &[extent.width, extent.height]);
            }
            Value::Texel(image) => {
                let extent = scope.extent(image);
                put(
                    offset,
                    &[
                        (1.0 / extent.width as f32).to_bits(),
                        (1.0 / extent.height as f32).to_bits(),
                    ],
                );
            }
            Value::Time(time) => put(offset, &[time.to_bits()]),
            Value::Frame => {
                let address = scope.frame();
                put(offset, &[address as u32, (address >> 32) as u32]);
            }
            Value::Parameters(at) => {
                let address = scope.addresses.effects + at;
                put(offset, &[address as u32, (address >> 32) as u32]);
            }
            Value::Missing => put(offset, &[NONE_INDEX]),
        }
    }
    Ok(push)
}

fn sampled_slot(
    scope: &PassScope<'_>,
    resources: &mut ResourceTable,
    image: ImageId,
) -> rikka_gpu::Result<u32> {
    let slot = scope.sampled(image);
    let bound = scope.bound(image);
    match bound.is_placed() {
        true => resources.place(bound.view, bound.sampler, bound.layout, bound.extent),
        false => Ok(slot),
    }
}
