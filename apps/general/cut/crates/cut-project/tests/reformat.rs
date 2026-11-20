use cut_project::{
    Appearance, Art, Edit, Error, Format, KeyChange, Marker, MarkerChange, MarkerScope,
    PlacementField, Point, Project, Property, Rate,
};

use crate::support::{applied, clip, film, imported, inserted, keyed, redrawn, square};

fn reformatted(project: &Project, width: u32, height: u32, rate: Rate) -> Project {
    applied(
        project,
        Edit::Reformat(Format {
            rate,
            width,
            height,
        }),
    )
}

#[test]
fn frame_rate_change_keeps_timing() {
    let (project, ids) = imported(vec![film("a", 4)]);
    assert_eq!(project.rate(), Rate::PAL);
    let opacity = Property::Placement(PlacementField::Opacity);
    let project = keyed(&project, ids[0], opacity.clone(), KeyChange::Add { at: 10 });
    let project = keyed(&project, ids[0], opacity.clone(), KeyChange::Add { at: 99 });
    let project = applied(
        &project,
        Edit::Mark {
            point: Point::In,
            at: 25,
        },
    );
    let project = applied(
        &project,
        Edit::Mark {
            point: Point::Out,
            at: 49,
        },
    );
    let project = applied(
        &project,
        Edit::Marker {
            on: MarkerScope::Timeline,
            change: MarkerChange::Add(Marker {
                length: 25,
                ..Marker::new(50)
            }),
        },
    );

    let project = reformatted(&project, 1280, 720, Rate::PAL_HIGH);

    let subject = clip(&project, ids[0]);
    assert_eq!((subject.start(), subject.length()), (0, 200));
    assert_eq!(subject.keyed_frames(), vec![20, 199]);
    let marks = project.marks();
    assert_eq!((marks.mark_in, marks.mark_out), (Some(50), Some(99)));
    let markers: Vec<(i64, i64)> = project
        .markers()
        .iter()
        .map(|(_, marker)| (marker.at, marker.length))
        .collect();
    assert_eq!(markers, vec![(100, 50)]);
}

#[test]
fn frame_size_change_keeps_composition() {
    let (project, ids) = imported(vec![film("a", 4)]);
    let (project, shape) = inserted(&project, square(100.0), 0, 30);
    let mut graphic = (**clip(&project, shape).graphic().expect("a graphic")).clone();
    graphic.placement.x = 100.0;
    graphic.placement.y = -50.0;
    let project = redrawn(&project, shape, Appearance::Graphic(graphic), 0);
    let zoom = clip(&project, ids[0]).placement().zoom;

    let project = reformatted(&project, 2560, 1440, Rate::PAL);

    let graphic = clip(&project, shape).graphic().expect("a graphic");
    assert_eq!((graphic.placement.x, graphic.placement.y), (200.0, -100.0));
    assert!(matches!(
        graphic.art,
        Art::Shape { width, height, .. } if width == 200.0 && height == 200.0
    ));
    assert_eq!(clip(&project, ids[0]).placement().zoom, zoom);
}

#[test]
fn explicit_format_survives_first_import() {
    let project = reformatted(&Project::new(), 1080, 1920, Rate::VIDEO);

    let project = applied(
        &project,
        Edit::Place {
            media: vec![film("a", 2)],
            at: 0,
            layer: 0,
        },
    );

    let format = project.format();
    assert_eq!(
        (format.width, format.height, format.rate),
        (1080, 1920, Rate::VIDEO)
    );
}

#[test]
fn unsupported_frame_sizes_are_rejected() {
    let edit = Edit::Reformat(Format {
        rate: Rate::VIDEO,
        width: 8,
        height: 1080,
    });

    assert!(matches!(
        Project::new().apply(&edit),
        Err(Error::FrameSize { width: 8, .. })
    ));
}
