use std::time::Duration;

use rikka_format::{format_bytes, format_duration, group_digits};
use rikka_i18n::tr;
use rikka_model::mmd::Vmd;
use rikka_model::{Error, Limit, Model};
use rikka_ui::Label;

pub(crate) fn figure(model: &Model) -> Label {
    tr!(
        "viewer.figure.detail",
        triangles = model.triangle_count(),
        materials = model.materials.len(),
    )
}

pub(crate) fn motion(motion: &Model) -> Label {
    let vmd = motion.extensions.get::<Vmd>();
    let nameless = tr!("viewer.figure.nameless");
    let model_name = vmd
        .map(|vmd| vmd.model.trim())
        .filter(|name| !name.is_empty())
        .unwrap_or(&nameless);
    let keyframes = vmd.map_or(0, |vmd| vmd.bones.len() + vmd.morphs.len());
    let length = format_duration(
        motion
            .animations
            .first()
            .map_or(Duration::ZERO, |animation| {
                Duration::from_secs_f32(animation.duration().max(0.0))
            }),
    );

    tr!(
        "viewer.figure.motion",
        model = model_name,
        keyframes = keyframes,
        length = &length,
    )
}

pub(crate) fn model_refusal(error: &Error) -> Option<Label> {
    match error {
        Error::UnsupportedVersion { format, version } => Some(tr!(
            "viewer.figure.unsupported_version",
            format = *format,
            version = version.as_str(),
        )),
        Error::RequiredExtensions(extensions) => Some(tr!(
            "viewer.figure.required_extensions",
            extensions = extensions.join(", ").as_str(),
        )),
        Error::LimitsExceeded {
            limit,
            actual,
            allowed,
        } => Some(limit_exceeded(*limit, *actual, *allowed)),
        _ => None,
    }
}

fn limit_exceeded(limit: Limit, actual: u64, allowed: u64) -> Label {
    let amount = |value: u64| match limit {
        Limit::FileBytes | Limit::ResourceBytes | Limit::TotalBytes => format_bytes(value),
        Limit::Vertices | Limit::Indices | Limit::Keyframes => group_digits(&value.to_string()),
    };
    let (actual, allowed) = (amount(actual), amount(allowed));
    let (actual, allowed) = (actual.as_str(), allowed.as_str());
    match limit {
        Limit::FileBytes => tr!(
            "viewer.figure.limit.file_bytes",
            actual = actual,
            allowed = allowed
        ),
        Limit::ResourceBytes => tr!(
            "viewer.figure.limit.resource_bytes",
            actual = actual,
            allowed = allowed
        ),
        Limit::TotalBytes => tr!(
            "viewer.figure.limit.total_bytes",
            actual = actual,
            allowed = allowed
        ),
        Limit::Vertices => tr!(
            "viewer.figure.limit.vertices",
            actual = actual,
            allowed = allowed
        ),
        Limit::Indices => tr!(
            "viewer.figure.limit.indices",
            actual = actual,
            allowed = allowed
        ),
        Limit::Keyframes => tr!(
            "viewer.figure.limit.keyframes",
            actual = actual,
            allowed = allowed
        ),
    }
}
