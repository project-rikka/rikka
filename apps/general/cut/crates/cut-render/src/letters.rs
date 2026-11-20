use cut_project::{Art, PresetTransform, Shot, letter_count};
use rikka_figure::{LetterPose, LetterWipe};

pub(crate) fn poses(shot: &Shot, art: &Art) -> Vec<LetterPose> {
    let Art::Text { lettering, reveal } = art else {
        return Vec::new();
    };
    if shot.letters.is_empty() && *reveal >= 1.0 {
        return Vec::new();
    }
    let count = letter_count(&lettering.text);
    let revealed_count = reveal * count as f32;
    let poses: Vec<LetterPose> = (0..count)
        .map(|index| {
            let transform = shot
                .letters
                .get(index)
                .copied()
                .unwrap_or(PresetTransform::REST);
            let revealed = (index as f32) < revealed_count && transform.reveal > 0.0;
            LetterPose {
                offset: transform.offset,
                zoom: transform.zoom,
                rotation: transform.rotation,
                opacity: match revealed {
                    true => transform.opacity,
                    false => 0.0,
                },
                blur: transform.blur,
                wipe: transform.wipe.map(|wipe| LetterWipe {
                    from: wipe.direction.vector(),
                    progress: wipe.progress,
                }),
            }
        })
        .collect();
    match poses.iter().all(LetterPose::is_rest) {
        true => Vec::new(),
        false => poses,
    }
}
