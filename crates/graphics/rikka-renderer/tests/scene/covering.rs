use rikka_renderer::{Node, Scene};
use rikka_types::{Color, Rect};

#[test]
fn painted_over_reports_visible_nodes_above() {
    let mut scene = Scene::new();
    let area = Rect::new(0.0, 100.0, 400.0, 300.0);
    let window = scene.insert(Node::rect(area).fill(Color::WHITE).z(10));
    scene.insert(Node::rect(Rect::new(0.0, 0.0, 400.0, 500.0)).z(5));
    scene.insert(Node::rect(Rect::new(500.0, 100.0, 50.0, 50.0)).z(20));
    scene.insert(
        Node::rect(Rect::new(100.0, 200.0, 50.0, 50.0))
            .z(20)
            .visible(false),
    );
    assert_eq!(scene.painted_over(window, area), None);

    scene.insert(Node::rect(Rect::new(-10.0, 380.0, 30.0, 30.0)).z(30));
    scene.insert(Node::rect(Rect::new(380.0, 390.0, 40.0, 40.0)).z(30));
    assert_eq!(
        scene.painted_over(window, area),
        Some(Rect::new(0.0, 380.0, 400.0, 20.0))
    );
}
