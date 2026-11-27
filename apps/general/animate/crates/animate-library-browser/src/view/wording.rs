use std::time::Duration;

use animate_library::{Details, Entry, Fit, Mismatch, Shelf, ThumbnailSize};
use rikka_app::prelude::*;
use rikka_format::format_duration;

pub(super) fn shelf(shelf: Shelf) -> Label {
    match shelf {
        Shelf::Models => tr!("animate.library.shelf.models"),
        Shelf::Motions => tr!("animate.library.shelf.motions"),
        Shelf::Cameras => tr!("animate.library.shelf.cameras"),
        Shelf::Stages => tr!("animate.library.shelf.stages"),
        Shelf::Accessories => tr!("animate.library.shelf.accessories"),
        Shelf::Music => tr!("animate.library.shelf.music"),
    }
}

pub(super) fn symbol(shelf: Shelf) -> MaterialSymbol {
    match shelf {
        Shelf::Models => MaterialSymbol::PERSON,
        Shelf::Motions => MaterialSymbol::ANIMATION,
        Shelf::Cameras => MaterialSymbol::VIDEOCAM,
        Shelf::Stages => MaterialSymbol::FOREST,
        Shelf::Accessories => MaterialSymbol::DIAMOND,
        Shelf::Music => MaterialSymbol::MUSIC_NOTE,
    }
}

pub(super) fn size(size: ThumbnailSize) -> Label {
    match size {
        ThumbnailSize::Small => tr!("animate.library.size.small"),
        ThumbnailSize::Medium => tr!("animate.library.size.medium"),
        ThumbnailSize::Large => tr!("animate.library.size.large"),
    }
}

pub(super) fn mismatch(mismatch: Mismatch) -> Label {
    match mismatch {
        Mismatch::Belongs(shelf) => tr!(
            "animate.library.belongs",
            shelf = self::shelf(shelf).to_string().as_str()
        ),
        Mismatch::Empty => tr!("animate.library.empty_file"),
        Mismatch::Unsupported => tr!("animate.library.unsupported"),
    }
}

pub(super) fn meta(entry: &Entry) -> Label {
    if let Fit::Mismatch(mismatch) = entry.fit() {
        return self::mismatch(mismatch);
    }
    let format = entry.format().name();
    match entry.details() {
        Details::Motion(motion) if motion.seconds > 0.0 => {
            Label::from(format!("{format} · {}", clock(motion.seconds)))
        }
        Details::Sound(sound) if sound.seconds > 0.0 => Label::from(clock(sound.seconds)),
        Details::Unreadable(_) => tr!("animate.library.unreadable"),
        _ => Label::from(format.to_owned()),
    }
}

pub(super) fn summary(entry: &Entry) -> Label {
    let mut lines = vec![entry.name().to_string()];
    match entry.details() {
        Details::Model(model) => {
            if let Some(title) = model
                .title
                .as_ref()
                .filter(|title| title.as_str() != &**entry.name())
            {
                lines.push(title.clone());
            }
            if let Some(author) = &model.author {
                lines.push(tr!("animate.library.author", name = author.as_str()).to_string());
            }
            if let Some(vertices) = model.vertices {
                lines.push(tr!("animate.library.vertices", count = vertices).to_string());
            }
            if let Some(vrm) = &model.vrm {
                let avatar = vrm.meta.avatar_permission.name();
                let commercial = vrm.meta.commercial_usage.name();
                lines.push(tr!(&format!("animate.library.avatar.{avatar}")).to_string());
                lines.push(tr!(&format!("animate.library.commercial.{commercial}")).to_string());
            }
            if let Some(comment) = model
                .comment
                .as_ref()
                .filter(|comment| !comment.trim().is_empty())
            {
                lines.push(rikka_format::truncate(comment.trim(), COMMENT).into_owned());
            }
        }
        Details::Motion(motion) => {
            let mut parts = Vec::new();
            if motion.body {
                parts.push(tr!("animate.library.has_body").to_string());
            }
            if motion.morphs {
                parts.push(tr!("animate.library.has_morphs").to_string());
            }
            if motion.camera {
                parts.push(tr!("animate.library.has_camera").to_string());
            }
            if !parts.is_empty() {
                lines.push(parts.join(" · "));
            }
            if let Some(frames) = motion.frames {
                lines.push(tr!("animate.library.frames", count = frames).to_string());
            }
            if let Some(target) = &motion.target {
                lines.push(tr!("animate.library.made_for", name = target.as_str()).to_string());
            }
        }
        Details::Sound(sound) => {
            for part in [&sound.title, &sound.artist, &sound.album]
                .into_iter()
                .flatten()
            {
                lines.push(part.clone());
            }
        }
        Details::Unreadable(reason) => lines.push(reason.to_string()),
        Details::Pending => {}
    }
    if let Fit::Mismatch(mismatch) = entry.fit() {
        lines.push(self::mismatch(mismatch).to_string());
    }
    if !entry.folder().is_empty() {
        lines.push(entry.folder().to_string());
    }
    Label::from(lines.join("\n"))
}

const COMMENT: usize = 160;

fn clock(seconds: f32) -> String {
    format_duration(Duration::from_secs_f32(seconds.max(0.0)))
}
