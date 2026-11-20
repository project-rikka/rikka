use cut_project::*;
use rikka_figure::{Band, Lettering, Look, Pen, Shadow, Shape, TextAlign};
use rikka_types::{Color, Point as Point2};

use crate::support::{film, imported, song};

fn placed(project: &Project, media: Vec<Media>, at: i64, layer: usize) -> (Project, Vec<ClipId>) {
    let (project, applied) = project
        .apply(&Edit::Place { media, at, layer })
        .expect("placing");
    (project, applied.selected.expect("the placed clips"))
}

fn spans(project: &Project, layer: usize) -> Vec<(i64, i64)> {
    project
        .layer(layer)
        .map(|layer| {
            layer
                .clips()
                .iter()
                .map(|clip| (clip.start(), clip.end()))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn video_places_one_clip_and_sets_format() {
    let (project, clips) = imported(vec![film("beach", 4)]);

    assert_eq!(
        project.format(),
        Format {
            rate: Rate::PAL,
            width: 1280,
            height: 720,
        }
    );
    assert_eq!(clips.len(), 1);
    assert_eq!(spans(&project, 0), [(0, 100)]);
    assert_eq!(project.voices_at(10.0).len(), 1);
    assert_eq!(
        project.visuals_at(10).last().map(|(_, clip, _)| clip.id()),
        Some(clips[0])
    );
}

#[test]
fn move_overwrites_and_grows_layers() {
    let (project, clips) = imported(vec![film("a", 4), film("b", 4), film("c", 4)]);

    let (moved, _) = project
        .apply(&Edit::Move {
            clips: vec![clips[2]],
            by: -150,
            across: 0,
        })
        .expect("moving");
    assert_eq!(spans(&moved, 0), [(0, 50), (50, 150), (150, 200)]);

    let (deep, _) = project
        .apply(&Edit::Move {
            clips: vec![clips[0]],
            by: 0,
            across: 9,
        })
        .expect("moving far down");
    assert_eq!(spans(&deep, 9), [(0, 100)]);
    assert_eq!(deep.layers().len(), 10);
    assert!(matches!(
        deep.apply(&Edit::Move {
            clips: vec![clips[1]],
            by: 0,
            across: -1,
        }),
        Err(Error::OutOfReach)
    ));
}

#[test]
fn trim_stops_at_media_and_neighbours() {
    let (project, clips) = imported(vec![film("a", 4), film("b", 4)]);
    let first = vec![clips[0]];
    let second = vec![clips[1]];

    assert_eq!(project.trim_limits(&first, Edge::Tail), -99..=0);
    assert_eq!(project.trim_limits(&first, Edge::Head), 0..=99);

    let (project, _) = project
        .apply(&Edit::Trim {
            clips: second.clone(),
            edge: Edge::Head,
            by: 30,
        })
        .expect("trimming the head");
    assert_eq!(spans(&project, 0), [(0, 100), (130, 200)]);
    assert_eq!(project.trim_limits(&second, Edge::Head), -30..=69);
    assert_eq!(project.locate(clips[1]).expect("there").1.source_in(), 30);
}

#[test]
fn split_cuts_clip_at_frame() {
    let (project, clips) = imported(vec![film("a", 4)]);

    let (project, _) = project
        .apply(&Edit::Split {
            clips: clips.clone(),
            at: 40,
        })
        .expect("splitting");

    assert_eq!(spans(&project, 0), [(0, 40), (40, 100)]);
    assert_eq!(
        project
            .visuals_at(40)
            .last()
            .map(|(_, clip, _)| clip.source_in()),
        Some(40)
    );
    assert!(matches!(
        project.apply(&Edit::Split { clips, at: 40 }),
        Err(Error::NothingToSplit)
    ));
}

#[test]
fn ripple_delete_needs_clear_layers() {
    let (project, clips) = imported(vec![film("a", 2), film("b", 2), film("c", 2)]);

    let (closed, _) = project
        .apply(&Edit::RippleDelete(vec![clips[1]]))
        .expect("rippling");
    assert_eq!(spans(&closed, 0), [(0, 50), (50, 100)]);

    let (crowded, _) = placed(&project, vec![song("d", 1)], 60, 1);
    let (lifted, _) = crowded
        .apply(&Edit::RippleDelete(vec![clips[1]]))
        .expect("rippling past a held layer");
    assert_eq!(spans(&lifted, 0), [(0, 50), (100, 150)]);
}

#[test]
fn locked_layers_reject_edits_and_placement() {
    let (project, clips) = imported(vec![film("a", 2)]);
    let (project, _) = project
        .apply(&Edit::Set {
            layer: 0,
            switch: Switch::Locked,
            on: true,
        })
        .expect("locking");

    assert!(matches!(
        project.apply(&Edit::Delete(clips.clone())),
        Err(Error::Locked)
    ));
    assert!(project.editable(&clips).is_empty());
    let (project, _) = placed(&project, vec![film("b", 1)], 100, 0);
    assert_eq!(spans(&project, 1), [(100, 125)]);
}

#[test]
fn layer_height_overrides_and_reset() {
    let (project, _) = imported(vec![film("a", 2)]);
    let (project, _) = project
        .apply(&Edit::Set {
            layer: 0,
            switch: Switch::Locked,
            on: true,
        })
        .expect("locking");
    let (resized, _) = project
        .apply(&Edit::Resize {
            layer: 0,
            height: Height::new(1000.0),
        })
        .expect("a locked layer still resizes");
    let (resized, _) = resized
        .apply(&Edit::Resize {
            layer: 12,
            height: Height::SMALL,
        })
        .expect("a layer beyond the last one resizes");

    assert_eq!(resized.height_of(0).px(), f32::from(*Height::RANGE.end()));
    assert_eq!(resized.height_of(12), Height::SMALL);
    assert_eq!(resized.height_of(1), Height::default());

    let (all, _) = resized
        .apply(&Edit::ResizeAll(Height::LARGE))
        .expect("resizing every layer");
    assert!((0..20).all(|layer| all.height_of(layer) == Height::LARGE));

    let unchanged = Edit::Resize {
        layer: 3,
        height: Height::default(),
    };
    assert_eq!(project.apply(&unchanged).expect("resizing").0, project);
}

#[test]
fn undo_redo_and_saved_state() {
    let mut document = Document::new();
    assert!(!document.is_modified());

    document
        .apply(&Edit::Place {
            media: vec![film("a", 2)],
            at: 0,
            layer: 0,
        })
        .expect("importing");
    let hide = Edit::Set {
        layer: 3,
        switch: Switch::Shown,
        on: false,
    };
    document.apply(&hide).expect("hiding");
    assert!(document.is_modified());
    assert_eq!(document.undoable(), Some(Action::Switch(Switch::Shown)));

    document.saved("/tmp/a.rcut".into(), document.revision());
    assert!(!document.is_modified());

    document.undo();
    assert!(document.is_modified());
    assert!(document.project().layers()[3].is_shown());

    document.redo();
    assert!(!document.is_modified());
    assert!(!document.project().layers()[3].is_shown());

    document.undo();
    document.apply(&Edit::ClearMarks).expect("nothing");
    assert_eq!(document.redoable(), Some(Action::Switch(Switch::Shown)));
    document
        .apply(&Edit::Mark {
            point: Point::In,
            at: 3,
        })
        .expect("branching");
    assert_eq!(document.redoable(), None);
}

#[test]
fn project_round_trips_through_file() {
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join(format!("story.{EXTENSION}"));
    let (project, clips) = imported(vec![film("a", 4), song("b", 2)]);
    let (project, _) = project
        .apply(&Edit::Split {
            clips: vec![clips[0]],
            at: 30,
        })
        .and_then(|(project, _)| {
            project.apply(&Edit::Move {
                clips: vec![clips[1]],
                by: 5,
                across: 3,
            })
        })
        .and_then(|(project, _)| {
            project.apply(&Edit::Mark {
                point: Point::In,
                at: 12,
            })
        })
        .expect("editing");

    save(&project, &path).expect("saving");
    let document = Document::open(&path).expect("opening");

    assert_eq!(**document.project(), project);
    assert_eq!(document.path(), Some(path.as_path()));
    assert!(!document.is_modified());
    let (grown, applied) = placed(document.project(), vec![song("c", 1)], 0, 0);
    let fresh = applied[0];
    assert!(
        grown.clips().filter(|(_, clip)| clip.id() == fresh).count() == 1
            && !project.clips().any(|(_, clip)| clip.id() == fresh)
    );
}

#[test]
fn save_keeps_permissions_and_symlink() {
    use std::os::unix::fs::PermissionsExt;

    let folder = tempfile::tempdir().expect("a folder");
    let real = folder.path().join(format!("real.{EXTENSION}"));
    let link = folder.path().join(format!("link.{EXTENSION}"));
    let (project, _) = imported(vec![film("a", 4)]);
    save(&project, &real).expect("saving");
    std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o640)).expect("permissions");
    std::os::unix::fs::symlink(&real, &link).expect("a symlink");

    save(&Project::new(), &link).expect("saving through the link");

    let linked = std::fs::symlink_metadata(&link).expect("the link");
    assert!(linked.file_type().is_symlink());
    let permissions = std::fs::metadata(&real).expect("the file").permissions();
    assert_eq!(permissions.mode() & 0o777, 0o640);
    assert_eq!(
        **Document::open(&real).expect("opening").project(),
        Project::new()
    );
}

#[test]
fn non_project_files_are_rejected() {
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join("odd.rcut");

    std::fs::write(&path, "{\"format\":\"rikka-cut\",\"version\":10}").expect("writing");
    assert!(matches!(
        Document::open(&path),
        Err(Error::File(rikka_file_format::Error::Decode(
            rikka_file_format::DecodeError::Version { found: 10, .. }
        )))
    ));

    std::fs::write(&path, "{\"format\":\"other\",\"version\":1}").expect("writing");
    assert!(matches!(
        Document::open(&path),
        Err(Error::File(rikka_file_format::Error::Decode(
            rikka_file_format::DecodeError::Foreign { .. }
        )))
    ));

    let (project, _) = imported(vec![film("a", 4)]);
    save(&project, &path).expect("saving");
    let text = std::fs::read_to_string(&path).expect("reading");
    let broken = text.replacen("\"start\": 0", "\"start\": -5", 1);
    std::fs::write(&path, broken).expect("writing");
    assert!(matches!(Document::open(&path), Err(Error::Damaged { .. })));
}

fn inserted(project: &Project, graphic: Graphic, at: i64, layer: usize) -> (Project, ClipId) {
    let (project, applied) = project
        .apply(&Edit::Insert {
            item: graphic.into(),
            at,
            length: 50,
            layer,
        })
        .expect("inserting");
    (project, applied.selected.expect("the inserted item")[0])
}

#[test]
fn graphics_round_trip_through_file() {
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join("captions.rcut");
    let (project, _) = imported(vec![film("beach", 4)]);
    let mut lettering = Lettering {
        text: "字幕".into(),
        size: 64.0,
        outlines: vec![Pen::new(Color::BLACK, 6.0), Pen::new(Color::WHITE, 3.0)],
        shadow: Some(Shadow {
            color: Color::rgba(0.0, 0.0, 0.0, 0.5),
            offset: Point2::new(4.0, 4.0),
            blur: 6.0,
        }),
        band: Some(Band {
            color: Color::rgba(0.1, 0.1, 0.1, 0.8),
            padding: 12.0,
            radius: 8.0,
        }),
        ..Lettering::default()
    };
    lettering.align = TextAlign::Center;
    let (project, _) = inserted(&project, Graphic::new(Art::text(lettering)), 0, 1);
    let (project, _) = inserted(
        &project,
        Graphic::new(Art::Shape {
            shape: Shape::Star {
                points: 7,
                depth: 0.4,
            },
            width: 80.0,
            height: 60.0,
            look: Look {
                fill: None,
                outline: Some(Pen::new(Color::rgb(1.0, 0.0, 0.0), 5.0)),
            },
        })
        .with_placement(Placement {
            x: -120.0,
            y: 40.0,
            zoom: 1.5,
            rotation: 30.0,
            opacity: 0.5,
            ..Placement::default()
        }),
        20,
        2,
    );

    save(&project, &path).expect("saving");
    let document = Document::open(&path).expect("opening");

    assert_eq!(**document.project(), project);
}

fn treated(project: &Project, clip: ClipId, change: impl FnOnce(&mut Treatment)) -> Project {
    let mut treatment = *project
        .locate(clip)
        .and_then(|(_, clip)| clip.treatment())
        .expect("footage");
    change(&mut treatment);
    project
        .apply(&Edit::Redraw {
            changes: vec![(clip, Appearance::Footage(treatment))],
            by: Redraw::Style,
            at: 0,
        })
        .expect("treating")
        .0
}

#[test]
fn split_speed_clip_keeps_source_mapping() {
    let (project, clips) = imported(vec![film("beach", 4)]);
    let project = treated(&project, clips[0], |treatment| treatment.speed = 1.5);
    let whole = project.locate(clips[0]).expect("there").1.source_at(21);
    let (project, _) = project
        .apply(&Edit::Split {
            clips: clips.clone(),
            at: 21,
        })
        .expect("splitting");
    let layer = &project.layers()[0];
    let (left, right) = (&layer.clips()[0], &layer.clips()[1]);

    assert!((left.origin() - right.origin()).abs() < 1.0);
    assert_eq!(right.source_at(21), whole);
    assert_eq!(right.treatment(), left.treatment());
}

#[test]
fn footage_settings_round_trip() {
    let folder = tempfile::tempdir().expect("a folder");
    let path = folder.path().join(format!("treated.{EXTENSION}"));
    let (project, clips) = imported(vec![film("beach", 4)]);
    let project = treated(&project, clips[0], |treatment| {
        *treatment = Treatment {
            placement: Placement {
                x: 40.0,
                y: -20.0,
                zoom: 0.5,
                rotation: 15.0,
                opacity: 0.75,
                ..Placement::default()
            },
            radius: 24.0,
            mirror: Mirror {
                horizontal: true,
                vertical: false,
            },
            speed: 1.25,
            mix: Mix {
                volume: 1.5,
                pan: -0.25,
                muted: true,
                fade_in: Fade {
                    length: 12,
                    shape: FadeShape::SCurve,
                },
                fade_out: Fade {
                    length: 6,
                    shape: FadeShape::Linear,
                },
            },
        }
    });

    save(&project, &path).expect("saving");
    let document = Document::open(&path).expect("opening");

    assert_eq!(**document.project(), project);
}
