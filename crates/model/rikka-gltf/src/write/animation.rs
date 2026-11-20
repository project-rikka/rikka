use crate::document::{
    Animation as DescribedAnimation, AnimationSampler, Channel, ChannelTarget, Document, Index,
    Interpolation as DescribedInterpolation, Path, Shape, Skin as DescribedSkin,
};
use crate::write::buffer::{Buffered, flatten};
use rikka_model_core::Model;
use rikka_model_core::{Interpolation, Track};

pub(crate) fn write(
    model: &Model,
    document: &mut Document,
    buffered: &mut Buffered,
) -> Vec<Option<Index>> {
    let mut skins = Vec::with_capacity(model.skins.len());
    for skin in &model.skins {
        if skin.joints.is_empty() {
            skins.push(None);
            continue;
        }
        document.skins.push(DescribedSkin {
            joints: skin.joints.iter().map(|joint| joint.0).collect(),
            inverse_bind_matrices: (!skin.inverse_bind.is_empty())
                .then(|| buffered.matrices(&skin.inverse_bind)),
            skeleton: skin.skeleton.map(|node| node.0),
            name: Some(skin.name.clone()).filter(|name| !name.is_empty()),
        });
        skins.push(Some((document.skins.len() - 1) as Index));
    }

    document.animations = model
        .animations
        .iter()
        .map(|animation| {
            let mut described = DescribedAnimation {
                channels: Vec::with_capacity(animation.channels.len()),
                samplers: Vec::with_capacity(animation.channels.len()),
                name: Some(animation.name.clone()).filter(|name| !name.is_empty()),
            };
            for channel in &animation.channels {
                let Some((sampler, path)) = written_track(&channel.track, buffered) else {
                    continue;
                };
                described.channels.push(Channel {
                    sampler: described.samplers.len() as Index,
                    target: ChannelTarget {
                        node: Some(channel.target.0),
                        path,
                    },
                });
                described.samplers.push(sampler);
            }
            described
        })
        .collect();
    skins
}

fn written_track(track: &Track, buffered: &mut Buffered) -> Option<(AnimationSampler, Path)> {
    let mut sampler =
        |input: &[f32], output: Vec<f32>, shape: Shape, interpolation: Interpolation| {
            AnimationSampler {
                input: buffered.times(input),
                output: buffered.plain(&output, shape),
                interpolation: match interpolation {
                    Interpolation::Linear => DescribedInterpolation::Linear,
                    Interpolation::Step => DescribedInterpolation::Step,
                    Interpolation::CubicSpline => DescribedInterpolation::CubicSpline,
                },
            }
        };
    match track {
        Track::Translation(curve) if !curve.times.is_empty() => Some((
            sampler(
                &curve.times,
                flatten(&curve.values),
                Shape::Vec3,
                curve.interpolation,
            ),
            Path::Translation,
        )),
        Track::Scale(curve) if !curve.times.is_empty() => Some((
            sampler(
                &curve.times,
                flatten(&curve.values),
                Shape::Vec3,
                curve.interpolation,
            ),
            Path::Scale,
        )),
        Track::Rotation(curve) if !curve.times.is_empty() => Some((
            sampler(
                &curve.times,
                curve
                    .values
                    .iter()
                    .flat_map(rikka_model_core::glam::Quat::to_array)
                    .collect(),
                Shape::Vec4,
                curve.interpolation,
            ),
            Path::Rotation,
        )),
        Track::Weights(weights) if !weights.times.is_empty() && !weights.values.is_empty() => {
            Some((
                sampler(
                    &weights.times,
                    weights.values.clone(),
                    Shape::Scalar,
                    weights.interpolation,
                ),
                Path::Weights,
            ))
        }
        _ => None,
    }
}
