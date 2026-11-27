use std::error::Error;

use animate_fluid::EmitterId;
use animate_prop::PropKind;
use animate_stage::StageError;
use rikka_format::{format_bytes, group_digits};
use rikka_i18n::tr;
use rikka_model::{Error as ModelError, Limit};
use rikka_ui::Label;

use crate::arrival::{ReadError, Rejection};

pub(crate) fn rejection(file: &str, rejection: &Rejection) -> Label {
    match rejection {
        Rejection::Unreadable(ReadError::Import(error)) => {
            model_refusal(file, error).unwrap_or_else(|| unreadable(file, error))
        }
        Rejection::Unreadable(error) => unreadable(file, error),
        Rejection::Unsupported => tr!("animate.refused.unsupported", file = file),
        Rejection::NoMotion => tr!("animate.refused.no_motion", file = file),
        Rejection::NoModel => tr!("animate.refused.no_model", file = file),
        Rejection::NeedsFigure => tr!("animate.refused.needs_figure", file = file),
        Rejection::NeedsCamera => tr!("animate.refused.needs_camera", file = file),
        Rejection::NeedsStage => tr!("animate.refused.needs_stage", file = file),
        Rejection::NeedsAccessory => tr!("animate.refused.needs_accessory", file = file),
        Rejection::NoSkeleton => tr!("animate.refused.no_skeleton", file = file),
        Rejection::BodyMismatch => tr!("animate.refused.body_mismatch", file = file),
        Rejection::Unmatched => tr!("animate.refused.unmatched", file = file),
    }
}

fn unreadable(file: &str, error: &dyn Error) -> Label {
    tr!(
        "animate.refused.unreadable",
        file = file,
        reason = cause(error).as_str()
    )
}

fn model_refusal(file: &str, error: &ModelError) -> Option<Label> {
    match error {
        ModelError::UnsupportedVersion { format, version } => Some(tr!(
            "animate.refused.unsupported_version",
            file = file,
            format = *format,
            version = version.as_str(),
        )),
        ModelError::RequiredExtensions(extensions) => Some(tr!(
            "animate.refused.required_extensions",
            file = file,
            extensions = extensions.join(", ").as_str(),
        )),
        ModelError::LimitsExceeded {
            limit,
            actual,
            allowed,
        } => Some(limit_exceeded(file, *limit, *actual, *allowed)),
        _ => None,
    }
}

fn limit_exceeded(file: &str, limit: Limit, actual: u64, allowed: u64) -> Label {
    let amount = |value: u64| match limit {
        Limit::FileBytes | Limit::ResourceBytes | Limit::TotalBytes => format_bytes(value),
        Limit::Vertices | Limit::Indices | Limit::Keyframes => group_digits(&value.to_string()),
    };
    let (actual, allowed) = (amount(actual), amount(allowed));
    let (actual, allowed) = (actual.as_str(), allowed.as_str());
    let key = match limit {
        Limit::FileBytes => "animate.refused.limit.file_bytes",
        Limit::ResourceBytes => "animate.refused.limit.resource_bytes",
        Limit::TotalBytes => "animate.refused.limit.total_bytes",
        Limit::Vertices => "animate.refused.limit.vertices",
        Limit::Indices => "animate.refused.limit.indices",
        Limit::Keyframes => "animate.refused.limit.keyframes",
    };
    tr!(key, file = file, actual = actual, allowed = allowed)
}

pub(crate) fn body_fit(file: &str, error: &dyn Error) -> Label {
    tr!(
        "animate.refused.body_fit",
        file = file,
        reason = cause(error).as_str()
    )
}

fn cause(error: &dyn Error) -> String {
    error
        .source()
        .map_or_else(|| error.to_string(), animate_ui::error_chain)
}

pub(crate) fn error(error: &StageError) -> Label {
    let reason = cause(error);
    let reason = reason.as_str();
    match error {
        StageError::Upload(_) | StageError::Drawing(_) | StageError::Readback => {
            tr!("animate.refused.drawing", reason = reason)
        }
        StageError::Liquid(_) => tr!("animate.refused.liquid", reason = reason),
        StageError::Cloth(_) => tr!("animate.refused.physics", reason = reason),
        StageError::Look(reason) => tr!("animate.refused.look", reason = reason.as_str()),
    }
}

pub(crate) fn physics(on: bool) -> Label {
    match on {
        false => tr!("animate.command.physics_off"),
        true => tr!("animate.command.physics_on"),
    }
}

pub(crate) fn liquid_name(id: EmitterId) -> String {
    tr!("animate.liquid.named", number = id.0 + 1).to_string()
}

pub(crate) fn prop_subject(kind: PropKind, name: &str) -> Label {
    match kind {
        PropKind::Stage => tr!("animate.subject.stage", name = name),
        PropKind::Accessory => tr!("animate.subject.accessory", name = name),
    }
}

pub(crate) fn copy_name(name: &str) -> String {
    tr!("animate.liquid.copy", name = name).to_string()
}
