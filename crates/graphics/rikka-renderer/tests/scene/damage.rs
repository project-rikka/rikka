use rikka_renderer::{Audience, GroupClip, Node, Scene, SceneFrame, Viewer};
use rikka_types::{Color, CornerRadii, Point, Rect, RoundedRect};

#[test]
fn idle_scene_frame_handover_is_empty() {
    let mut source = Scene::new();
    let mut frame = SceneFrame::default();
    source.take_frame(&mut frame);
    assert!(frame.is_empty(), "untouched scene emits an empty frame");

    let mut mirror = Scene::new();
    let id = source.insert(Node::rect(Rect::new(0.0, 0.0, 20.0, 20.0)).fill(Color::WHITE));
    source.take_frame(&mut frame);
    assert!(!frame.is_empty());
    mirror.apply(&mut frame);

    source.take_frame(&mut frame);
    assert!(frame.is_empty(), "idle frame after edit must be empty");

    source.edit(id).audience(Audience::Only(Viewer::Screen));
    source.take_frame(&mut frame);
    assert!(!frame.is_empty());
    mirror.apply(&mut frame);

    source.take_frame(&mut frame);
    assert!(frame.is_empty(), "subsequent idle frame stays empty");
}

#[test]
fn scene_frame_reused_group_and_layer_slots_apply_cleanly() {
    let mut source = Scene::new();
    let mut mirror = Scene::new();
    let mut frame = SceneFrame::default();

    let group_a = source.create_group();
    let layer_a = source.create_layer();
    source.set_group(
        group_a,
        None,
        Point::new(10.0, 20.0),
        Rect::new(0.0, 0.0, 100.0, 100.0),
    );
    source.set_group_clip(
        group_a,
        GroupClip::Narrow(RoundedRect::rect(Rect::new(0.0, 0.0, 50.0, 50.0))),
    );
    source.set_layer(layer_a, None, 0.5);
    source.take_frame(&mut frame);
    mirror.apply(&mut frame);

    source.remove_group(group_a);
    let group_b = source.create_group();
    source.set_group(
        group_b,
        None,
        Point::new(30.0, 40.0),
        Rect::new(0.0, 0.0, 80.0, 80.0),
    );
    source.remove_layer(layer_a);
    let layer_b = source.create_layer();
    source.set_layer(layer_b, None, 0.25);
    let node = source.insert(
        Node::rect(Rect::new(0.0, 0.0, 10.0, 10.0))
            .group(group_b)
            .layer(layer_b)
            .fill(Color::WHITE),
    );
    source.take_frame(&mut frame);
    mirror.apply(&mut frame);

    assert!(mirror.get(node).is_some());
    assert!(mirror.has_damage());
}

#[test]
fn hidden_node_edits_produce_no_damage() {
    let mut scene = Scene::new();
    let mut frame = SceneFrame::default();
    let hidden = scene.insert(
        Node::rounded_rect(Rect::new(0.0, 0.0, 40.0, 40.0), 4.0)
            .fill(Color::WHITE)
            .visible(false),
    );
    scene.take_frame(&mut frame);
    assert!(!scene.has_damage());

    scene
        .edit(hidden)
        .fill(Color::BLACK)
        .rect(Rect::new(10.0, 10.0, 40.0, 40.0))
        .radii(CornerRadii::uniform(8.0));
    assert!(!scene.has_damage(), "editing a hidden node does not damage");

    scene.edit(hidden).visible(true);
    assert!(scene.has_damage(), "showing the node damages its area");
}

#[test]
fn invisible_layer_fade_in_produces_damage() {
    let mut scene = Scene::new();
    let mut frame = SceneFrame::default();
    let layer = scene.create_layer();
    scene.set_layer(layer, None, 0.0);
    scene.insert(
        Node::rect(Rect::new(10.0, 10.0, 40.0, 40.0))
            .layer(layer)
            .fill(Color::WHITE),
    );
    scene.take_frame(&mut frame);
    assert!(!scene.has_damage());

    scene.set_layer(layer, None, 0.5);
    assert!(
        scene.has_damage(),
        "fading in a layer initialized at opacity 0.0 must damage its members"
    );
}
