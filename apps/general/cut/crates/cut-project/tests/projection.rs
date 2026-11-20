use crate::support::{clip, close, inserted, near, near_point, square};
use cut_project::{
    Camera, ClipId, Edit, Format, Graphic, Homography, Item, Placement, Project, Region, Viewpoint,
};
use rikka_figure::{Affine, Shape, enclosing};
use rikka_types::{Point, Rect};

const FOCAL: f32 = 2_666.502_6;

fn placed(project: &Project, item: impl Into<Item>, layer: usize) -> (Project, ClipId) {
    let (project, applied) = project
        .apply(&Edit::Insert {
            item: item.into(),
            at: 0,
            length: 30,
            layer,
        })
        .expect("inserting");
    (project, applied.selected.expect("the item")[0])
}

fn card(placement: Placement) -> Graphic {
    square(100.0).with_placement(placement)
}

fn projected(project: &Project, id: ClipId, local: Point) -> Option<Point> {
    project
        .shot(clip(project, id), 10, Viewpoint::Output)
        .expect("a shot")
        .projection
        .map(local)
}

#[test]
fn screen_points_unproject_to_item_plane() {
    let placement = Placement {
        x: -80.0,
        y: 60.0,
        depth: 300.0,
        tilt_x: 25.0,
        tilt_y: -35.0,
        rotation: 12.0,
        zoom: 0.8,
        three_d: true,
        ..Placement::default()
    };
    let (project, camera) = placed(
        &Project::new(),
        Item::Camera(Camera {
            interest: Point::new(40.0, -30.0),
            pan: 20.0,
            tilt: -10.0,
            roll: 5.0,
            ..Camera::default()
        }),
        3,
    );
    let (project, id) = placed(&project, card(placement), 0);
    let shot = project
        .shot(clip(&project, id), 10, Viewpoint::Output)
        .expect("a shot");
    assert_eq!(shot.cameras, [camera]);

    for local in [
        Point::new(-50.0, -50.0),
        Point::new(33.0, 12.0),
        Point::new(50.0, 49.0),
    ] {
        let screen = shot.projection.map(local).expect("in front");
        let back = shot.projection.unmap(screen).expect("on the plane");
        assert!(
            near_point(back, local, 1e-2),
            "{local:?} came back as {back:?}"
        );
        let total = shot.projection.homography().apply(local).expect("a point");
        assert!(near_point(total, screen, 1e-2));
    }
}

#[test]
fn points_behind_camera_are_hidden() {
    let (project, id) = placed(
        &Project::new(),
        card(Placement {
            depth: -2.0 * FOCAL,
            three_d: true,
            ..Placement::default()
        }),
        0,
    );

    assert_eq!(projected(&project, id, Point::ZERO), None);
    let shot = project
        .shot(clip(&project, id), 10, Viewpoint::Output)
        .expect("a shot");
    assert_eq!(shot.projection.unmap(Point::new(960.0, 540.0)), None);
}

#[test]
fn cameras_affect_layers_below_and_stack() {
    let (project, below) = placed(&Project::new(), card(Placement::default()), 0);
    let (project, near_camera) = placed(
        &project,
        Item::Camera(Camera {
            zoom: 2.0,
            ..Camera::default()
        }),
        1,
    );
    let (project, between) = placed(&project, card(Placement::default()), 2);
    let (project, far_camera) = placed(
        &project,
        Item::Camera(Camera {
            interest: Point::new(50.0, 0.0),
            ..Camera::default()
        }),
        3,
    );
    let (project, above) = placed(&project, card(Placement::default()), 4);
    let corner = Point::new(50.0, 50.0);

    assert!(near_point(
        projected(&project, above, corner).expect("a point"),
        Point::new(1010.0, 590.0),
        1e-2
    ));
    assert!(near_point(
        projected(&project, between, corner).expect("a point"),
        Point::new(960.0, 590.0),
        1e-2
    ));
    assert!(near_point(
        projected(&project, below, corner).expect("a point"),
        Point::new(1010.0, 640.0),
        1e-2
    ));
    let shot = project
        .shot(clip(&project, below), 10, Viewpoint::Output)
        .expect("a shot");
    assert_eq!(shot.cameras, [near_camera, far_camera]);

    let overview = Viewpoint::Overview {
        camera: near_camera,
        scale: 0.5,
    };
    let shot = project
        .shot(clip(&project, below), 10, overview)
        .expect("a shot");
    assert!(near_point(
        shot.projection.map(corner).expect("a point"),
        Point::new(985.0, 565.0),
        1e-2
    ));
    let quad = project.camera_view(near_camera, 10, 1.0).expect("a view");
    assert!(near_point(quad[0], Point::new(480.0, 270.0), 1e-2));
    assert!(near_point(quad[2], Point::new(1440.0, 810.0), 1e-2));
    let wide = project.camera_view(far_camera, 10, 0.5).expect("a view");
    assert!(near_point(wide[0], Point::new(505.0, 270.0), 1e-2));
}

#[test]
fn homography_compose_invert_and_matrix() {
    let affine = Affine::rotation(30.0).then(Affine::translation(5.0, -7.0));
    let map = Homography::from_affine(affine);
    let rows = map.matrix();

    assert_eq!(rows[2], [0.0, 0.0, 1.0]);
    assert!(near(rows[0][2], 5.0, 1e-6) && near(rows[1][2], -7.0, 1e-6));
    let point = Point::new(3.0, 4.0);
    assert!(near_point(
        map.apply(point).expect("a point"),
        affine.apply(point),
        1e-5
    ));
    let back = map.inverse().expect("an inverse");
    assert!(near_point(
        map.then(back).apply(point).expect("a point"),
        point,
        1e-5
    ));
    assert_eq!(map.affine().map(|affine| affine.tx), Some(5.0));
    let scaled = Homography::about(Point::new(10.0, 10.0), 0.5);
    assert_eq!(
        scaled.apply(Point::new(20.0, 10.0)),
        Some(Point::new(15.0, 10.0))
    );
}

fn corners(side: f32) -> [Point; 4] {
    let half = side / 2.0;
    [
        Point::new(-half, -half),
        Point::new(half, -half),
        Point::new(half, half),
        Point::new(-half, half),
    ]
}

fn frame() -> Rect {
    Rect::new(0.0, 0.0, 1920.0, 1080.0)
}

fn shot_of(placement: Placement) -> cut_project::Shot {
    let (project, id) = inserted(
        &Project::new(),
        square(1000.0).with_placement(placement),
        0,
        30,
    );
    project
        .shot(clip(&project, id), 0, Viewpoint::Output)
        .expect("a shot")
}

#[test]
fn clipping_keeps_visible_polygon() {
    let flat = shot_of(Placement::default());
    assert!(!flat.projection.crosses_near_plane(&corners(400.0)));
    assert_eq!(flat.projection.clipped(&corners(400.0), frame()).len(), 4);
    let wide = flat.projection.clipped(&corners(4000.0), frame());
    for local in &wide {
        let projected = flat.projection.map(*local).expect("in front");
        assert!(frame().inflate(0.01).contains(projected), "{projected:?}");
    }

    let shots = shot_of(Placement {
        x: 200.0,
        depth: -2300.0,
        tilt_y: 75.0,
        three_d: true,
        ..Placement::default()
    });
    assert!(shots.projection.crosses_near_plane(&corners(1000.0)));
    let clipped = shots.projection.clipped(&corners(1000.0), frame());
    assert!(clipped.len() >= 3);
    for local in &clipped {
        let projected = shots.projection.map(*local).expect("in front");
        assert!(frame().inflate(0.01).contains(projected), "{projected:?}");
    }
}

fn banner() -> Region {
    Region::shaped(Shape::Rectangle { radius: 0.0 }, 200.0, 100.0).with_placement(Placement {
        x: 100.0,
        zoom: 2.0,
        ..Placement::default()
    })
}

fn points(region: &Region, format: Format) -> Vec<Point> {
    region
        .path(format)
        .expect("a path")
        .subpaths()
        .iter()
        .flat_map(|subpath| subpath.points().iter().copied())
        .collect()
}

#[test]
fn shaped_region_path_in_project_pixels() {
    let format = Format::default();

    let bounds = enclosing(points(&banner(), format));

    assert!(close(bounds.min_x(), 860.0) && close(bounds.max_x(), 1260.0));
    assert!(close(bounds.min_y(), 440.0) && close(bounds.max_y(), 640.0));
}
