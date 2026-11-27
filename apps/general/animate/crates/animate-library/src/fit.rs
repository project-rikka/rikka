use crate::details::{Details, DocumentKind};
use crate::format::Format;
use crate::shelf::Shelf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fit {
    #[default]
    Unknown,
    Fits,
    Mismatch(Mismatch),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mismatch {
    Belongs(Shelf),
    Empty,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Default)]
struct Content {
    model: bool,
    motion: bool,
    camera: bool,
    sound: bool,
    unsupported: bool,
}

impl Fit {
    pub(crate) fn judge(shelf: Shelf, format: Format, details: &Details) -> Fit {
        let Some(content) = Content::from_details(format, details) else {
            return Fit::Unknown;
        };
        let fits = match shelf {
            Shelf::Models | Shelf::Stages | Shelf::Accessories => content.model,
            Shelf::Motions => content.motion,
            Shelf::Cameras => content.camera,
            Shelf::Music => content.sound,
        };
        let belongs = [
            (content.model, Shelf::Models),
            (content.motion, Shelf::Motions),
            (content.camera, Shelf::Cameras),
            (content.sound, Shelf::Music),
        ]
        .into_iter()
        .find_map(|(held, shelf)| held.then_some(shelf));
        match (fits, belongs, content.unsupported) {
            (true, _, _) => Fit::Fits,
            (false, Some(shelf), _) => Fit::Mismatch(Mismatch::Belongs(shelf)),
            (false, None, true) => Fit::Mismatch(Mismatch::Unsupported),
            (false, None, false) => Fit::Mismatch(Mismatch::Empty),
        }
    }
}

impl Content {
    fn from_details(format: Format, details: &Details) -> Option<Content> {
        let mut content = Content::default();
        match (format, details) {
            (_, Details::Unreadable(_)) => return None,
            (Format::Pmx | Format::Vrm | Format::Blend | Format::Stl, _) => content.model = true,
            (_, Details::Pending) => return None,
            (Format::Sound(_), _) => content.sound = true,
            (Format::Vrma | Format::Fbx | Format::Gltf | Format::Glb, Details::Model(model)) => {
                content.model = model.meshes.is_some_and(|meshes| meshes > 0);
                content.motion = !model.clips.is_empty();
            }
            (Format::Vrma | Format::Fbx | Format::Gltf | Format::Glb, Details::Motion(motion)) => {
                content.motion = !motion.clips.is_empty();
            }
            (Format::Vmd | Format::Motion, Details::Motion(motion)) => match motion.document {
                Some(DocumentKind::Figure) => content.motion = true,
                Some(DocumentKind::Camera) => content.camera = true,
                Some(
                    DocumentKind::Look
                    | DocumentKind::Fluid
                    | DocumentKind::Wind
                    | DocumentKind::Stage
                    | DocumentKind::Accessory,
                ) => {
                    content.unsupported = true;
                }
                None => {
                    content.motion = motion.body || motion.morphs;
                    content.camera = motion.camera;
                }
            },
            _ => return None,
        }
        Some(content)
    }
}
