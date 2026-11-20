use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::support::{clip, film, imported, song};
use cut_project::{
    Catalog, Document, Edit, Error, Media, Mismatch, Picture, Rate, Relink, Relocation,
};

fn moved(media: &Media, path: &str) -> Media {
    let mut to = Media::new(path, media.duration());
    if let Some(picture) = media.picture() {
        to = to.with_picture(picture);
    }
    match media.sound() {
        Some(sound) => to.with_sound(sound),
        None => to,
    }
}

fn silent_film(seconds: u64) -> Media {
    Media::new("/films/silent.mp4", Duration::from_secs(seconds)).with_picture(Picture {
        width: 1280,
        height: 720,
        rate: Rate::PAL,
    })
}

#[test]
fn relinked_media_serves_every_clip_and_undoes() {
    let mut document = Document::new();
    let placed = document
        .apply(&Edit::Place {
            media: vec![film("beach", 4)],
            at: 0,
            layer: 0,
        })
        .expect("placing")
        .selected
        .expect("the placed clip");
    document
        .apply(&Edit::Split {
            clips: placed.clone(),
            at: 40,
        })
        .expect("splitting");
    let project = Arc::clone(document.project());
    let media = clip(&project, placed[0]).media().expect("footage");
    let original = project.media(media).expect("media").clone();
    let to = moved(&original, "/backup/beach.mp4");
    assert_eq!(project.uses(media), 2);

    document
        .apply(&Edit::Relink(vec![Relink {
            media,
            to: to.clone(),
        }]))
        .expect("relinking");
    let relinked = document.project();
    assert!(
        relinked
            .clips()
            .all(|(_, clip)| clip.media() == Some(media))
    );
    assert_eq!(relinked.media(media), Some(&to));

    document.undo();
    assert_eq!(document.project().media(media), Some(&original));
}

#[test]
fn unfit_replacements_are_refused() {
    let (project, clips) = imported(vec![film("beach", 4), song("tune", 4)]);
    let beach = clip(&project, clips[0]).media().expect("footage");
    let tune = clip(&project, clips[1]).media().expect("footage");
    let cases = [
        (beach, song("other", 4), Mismatch::MissingPicture),
        (beach, silent_film(4), Mismatch::MissingSound),
        (tune, film("other", 4), Mismatch::UnexpectedPicture),
        (
            beach,
            film("short", 2),
            Mismatch::TooShort {
                needed: 100,
                frames: 50,
            },
        ),
    ];
    for (media, to, expected) in cases {
        let fit = project.relink_fit(media, &to);
        assert!(
            matches!(fit, Err(Error::Mismatch { reason, .. }) if reason == expected),
            "{expected:?}"
        );
        let refused = project.apply(&Edit::Relink(vec![Relink { media, to }]));
        assert!(
            matches!(refused, Err(Error::Mismatch { reason, .. }) if reason == expected),
            "{expected:?}"
        );
    }
    assert!(project.relink_fit(beach, &film("longer", 9)).is_ok());
    let unknown = cut_project::Project::new().relink_fit(beach, &film("beach", 4));
    assert!(matches!(unknown, Err(Error::UnknownMedia)));
}

#[test]
fn relocation_rebases_moved_folders() {
    let relocation = Relocation::between(
        Path::new("/old/shoot/day1/a.mp4"),
        Path::new("/mnt/disk/shoot/day1/a.mp4"),
    )
    .expect("a relocation");
    let cases = [
        (
            "/old/shoot/day2/b.mp4",
            vec!["/mnt/disk/shoot/day2/b.mp4", "/mnt/disk/shoot/day1/b.mp4"],
        ),
        ("/elsewhere/c.mp4", vec!["/mnt/disk/shoot/day1/c.mp4"]),
        ("/old/shoot/day1/d.mp4", vec!["/mnt/disk/shoot/day1/d.mp4"]),
    ];
    for (original, expected) in cases {
        let expected: Vec<PathBuf> = expected.into_iter().map(PathBuf::from).collect();
        assert_eq!(
            relocation.candidates(Path::new(original)),
            expected,
            "{original}"
        );
    }
}

#[test]
fn catalog_finds_names_nearest_folder_first() {
    let root = tempfile::tempdir().expect("a folder");
    let files = [
        "x/day1/A001.mp4",
        "y/day2/a001.MP4",
        ".trash/day1/A001.mp4",
        "z/other.mp4",
    ];
    for file in files {
        let path = root.path().join(file);
        fs::create_dir_all(path.parent().expect("a parent")).expect("folders");
        fs::write(&path, b"").expect("a file");
    }

    let wanted = [
        Path::new("/old/card/day2/A001.mp4"),
        Path::new("/old/missing.mp4"),
    ];
    let catalog = Catalog::scan(root.path(), wanted).expect("scanning");
    let found = catalog.find(wanted[0]);

    let expected = [
        root.path().join("y/day2/a001.MP4"),
        root.path().join("x/day1/A001.mp4"),
    ];
    assert_eq!(found, expected);
    assert!(catalog.find(wanted[1]).is_empty());
    assert!(catalog.find(Path::new("/old/z/other.mp4")).is_empty());
    assert!(Catalog::scan(&root.path().join("absent"), wanted).is_err());
}
