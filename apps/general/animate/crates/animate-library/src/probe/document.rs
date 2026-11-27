use std::path::Path;

use animate_document::Motion;
use rikka_model::mmd::FRAMES_PER_SECOND;

use crate::details::{Details, DocumentKind, MotionDetails};

use super::fault::{Fault, Probed};

const LARGEST: u64 = 256 << 20;

pub(super) fn read(path: &Path) -> Probed<Details> {
    if std::fs::metadata(path)?.len() > LARGEST {
        return Err(Fault::TooLarge);
    }
    let motion = Motion::load(path).map_err(|error| Fault::Import(error.to_string()))?;
    let details = match &motion {
        Motion::Figure(take) => MotionDetails {
            frames: Some(take.last_frame()),
            body: take.human.as_ref().is_some_and(|human| !human.is_empty())
                || !take.bones.is_empty(),
            morphs: !take.morphs.is_empty(),
            document: Some(DocumentKind::Figure),
            ..MotionDetails::default()
        },
        Motion::Camera(camera) => MotionDetails {
            frames: Some(camera.last_frame()),
            camera: !camera.is_empty(),
            document: Some(DocumentKind::Camera),
            ..MotionDetails::default()
        },
        Motion::Look(_) => MotionDetails {
            document: Some(DocumentKind::Look),
            ..MotionDetails::default()
        },
        Motion::Fluid(_) => MotionDetails {
            document: Some(DocumentKind::Fluid),
            ..MotionDetails::default()
        },
        Motion::Wind(_) => MotionDetails {
            document: Some(DocumentKind::Wind),
            ..MotionDetails::default()
        },
        Motion::Stage(_) => MotionDetails {
            document: Some(DocumentKind::Stage),
            ..MotionDetails::default()
        },
        Motion::Accessory(_) => MotionDetails {
            document: Some(DocumentKind::Accessory),
            ..MotionDetails::default()
        },
    };
    Ok(Details::Motion(Box::new(MotionDetails {
        seconds: details.frames.unwrap_or(0) as f32 / FRAMES_PER_SECOND,
        ..details
    })))
}
