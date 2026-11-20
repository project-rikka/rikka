use std::time::Duration;

use cut_project::{
    Appearance, Art, Clip, ClipId, Edit, Graphic, Item, KeyChange, Media, Picture, Project,
    Property, Rate, Redraw, Sound,
};
use rikka_figure::{Look, Shape};
use rikka_types::{Color, Point};

pub(crate) fn film(name: &str, seconds: u64) -> Media {
    Media::new(format!("/films/{name}.mp4"), Duration::from_secs(seconds))
        .with_picture(Picture {
            width: 1280,
            height: 720,
            rate: Rate::PAL,
        })
        .with_sound(Sound {
            sample_rate: 48_000,
            channels: 2,
        })
}

pub(crate) fn song(name: &str, seconds: u64) -> Media {
    Media::new(format!("/songs/{name}.flac"), Duration::from_secs(seconds)).with_sound(Sound {
        sample_rate: 44_100,
        channels: 2,
    })
}

pub(crate) fn imported(media: Vec<Media>) -> (Project, Vec<ClipId>) {
    let (project, applied) = Project::new()
        .apply(&Edit::Place {
            media,
            at: 0,
            layer: 0,
        })
        .expect("placing");
    (project, applied.selected.expect("the placed clips"))
}

pub(crate) fn square(side: f32) -> Graphic {
    Graphic::new(Art::Shape {
        shape: Shape::Rectangle { radius: 0.0 },
        width: side,
        height: side,
        look: Look {
            fill: Some(Color::WHITE),
            outline: None,
        },
    })
}

pub(crate) fn inserted(
    project: &Project,
    item: impl Into<Item>,
    at: i64,
    length: i64,
) -> (Project, ClipId) {
    let (project, applied) = project
        .apply(&Edit::Insert {
            item: item.into(),
            at,
            length,
            layer: 0,
        })
        .expect("inserting");
    (project, applied.selected.expect("the inserted item")[0])
}

pub(crate) fn applied(project: &Project, edit: Edit) -> Project {
    project.apply(&edit).expect("applying").0
}

pub(crate) fn clip(project: &Project, id: ClipId) -> &Clip {
    project.locate(id).expect("the clip is there").1
}

pub(crate) fn clips_on(project: &Project, layer: usize) -> Vec<&Clip> {
    project.layers()[layer].clips().iter().collect()
}

pub(crate) fn redrawn(project: &Project, id: ClipId, appearance: Appearance, at: i64) -> Project {
    applied(
        project,
        Edit::Redraw {
            changes: vec![(id, appearance)],
            by: Redraw::Move,
            at,
        },
    )
}

pub(crate) fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

pub(crate) fn near(left: f32, right: f32, slack: f32) -> bool {
    (left - right).abs() <= slack
}

pub(crate) fn near_point(left: Point, right: Point, slack: f32) -> bool {
    left.distance(right) <= slack
}

pub(crate) fn keyed(
    project: &Project,
    id: ClipId,
    property: Property,
    change: KeyChange,
) -> Project {
    applied(
        project,
        Edit::Key {
            clip: id,
            property,
            change,
        },
    )
}
