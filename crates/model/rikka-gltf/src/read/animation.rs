use rikka_model_core::glam::Mat4;

use crate::document::{self, Document, Interpolation as SourceDocument, Path};
use crate::read::accessor::Accessors;
use rikka_model_core::ImportCtx;
use rikka_model_core::ModelBuilder;
use rikka_model_core::NodeId;
use rikka_model_core::Skin;
use rikka_model_core::Warning;
use rikka_model_core::{Animation, Channel, Curve, Interpolation, Track, Weights};
use rikka_model_core::{Error, Result};

pub(crate) fn read_skins(
    document: &Document,
    accessors: &Accessors<'_>,
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<()> {
    for (index, described) in document.skins.iter().enumerate() {
        ctx.checkpoint()?;
        let name = described
            .name
            .clone()
            .unwrap_or_else(|| format!("skin{index}"));
        let joints = named_nodes(document, &described.joints, &name)?;
        let inverse_bind = match described.inverse_bind_matrices {
            Some(accessor) => accessors.matrices(accessor)?,
            None => vec![Mat4::IDENTITY; joints.len()],
        };
        if inverse_bind.len() < joints.len() {
            return Err(Error::Damaged(format!(
                "skin \"{name}\" binds {} joints with {} matrices",
                joints.len(),
                inverse_bind.len()
            )));
        }
        builder.add_skin(Skin {
            name,
            inverse_bind: inverse_bind[..joints.len()].to_vec(),
            joints,
            skeleton: described
                .skeleton
                .filter(|node| (*node as usize) < document.nodes.len())
                .map(NodeId),
        });
    }
    Ok(())
}

fn named_nodes(document: &Document, joints: &[u32], name: &str) -> Result<Vec<NodeId>> {
    joints
        .iter()
        .map(|joint| match (*joint as usize) < document.nodes.len() {
            true => Ok(NodeId(*joint)),
            false => Err(Error::Damaged(format!(
                "skin \"{name}\" references missing node {joint}"
            ))),
        })
        .collect()
}

pub(crate) fn read_animations(
    document: &Document,
    accessors: &Accessors<'_>,
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<()> {
    for (index, described) in document.animations.iter().enumerate() {
        ctx.checkpoint()?;
        let name = described
            .name
            .clone()
            .unwrap_or_else(|| format!("animation{index}"));
        let mut animation = Animation::new(name.clone());
        for (slot, channel) in described.channels.iter().enumerate() {
            let what = format!("channel {slot} of animation \"{name}\"");
            if let Some(read) = read_channel(document, accessors, described, channel, &what, ctx)? {
                animation.channels.push(read);
            }
        }
        if !animation.channels.is_empty() {
            builder.add_animation(animation);
        }
    }
    Ok(())
}

fn read_channel(
    document: &Document,
    accessors: &Accessors<'_>,
    described: &document::Animation,
    channel: &document::Channel,
    what: &str,
    ctx: &mut ImportCtx<'_>,
) -> Result<Option<Channel>> {
    let Some(target) = channel.target.node else {
        return Ok(None);
    };
    if target as usize >= document.nodes.len() {
        ctx.warn(Warning::Repaired(format!(
            "{what} targets missing node {target}"
        )));
        return Ok(None);
    }
    let Some(sampler) = described.samplers.get(channel.sampler as usize) else {
        ctx.warn(Warning::Repaired(format!(
            "{what} references missing sampler {}",
            channel.sampler
        )));
        return Ok(None);
    };

    let times = accessors.scalars(sampler.input)?;
    if times.is_empty() {
        return Ok(None);
    }
    ctx.charge_keyframes(times.len())?;
    let interpolation = match sampler.interpolation {
        SourceDocument::Linear => Interpolation::Linear,
        SourceDocument::Step => Interpolation::Step,
        SourceDocument::CubicSpline => Interpolation::CubicSpline,
    };
    let expected = times.len() * interpolation.stride();
    let values = accessors.count(sampler.output)?;

    let track = match channel.target.path {
        Path::Translation | Path::Scale | Path::Rotation if values != expected => {
            ctx.warn(Warning::Repaired(format!(
                "skipping {what}: {values} values for {expected} keyframes"
            )));
            return Ok(None);
        }
        Path::Translation => Track::Translation(Curve {
            times,
            values: accessors.vec3(sampler.output)?,
            interpolation,
        }),
        Path::Scale => Track::Scale(Curve {
            times,
            values: accessors.vec3(sampler.output)?,
            interpolation,
        }),
        Path::Rotation => Track::Rotation(Curve {
            times,
            values: accessors.quats(sampler.output)?,
            interpolation,
        }),
        Path::Weights => {
            let read = accessors.scalars(sampler.output)?;
            if expected == 0 || !read.len().is_multiple_of(expected) {
                ctx.warn(Warning::Repaired(format!(
                    "skipping {what}: {} morph weights for {expected} keyframes",
                    read.len()
                )));
                return Ok(None);
            }
            Track::Weights(Weights {
                count: read.len() / expected,
                times,
                values: read,
                interpolation,
            })
        }
    };

    Ok(Some(Channel {
        target: NodeId(target),
        track,
    }))
}
