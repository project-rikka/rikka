use std::sync::Arc;

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{
    Color, Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Rect, ScrollDelta, Size,
};
use rikka_ui::{Element, Event, ScrollPhase, ScrollSource, State, Styled, TypedUi, View};
use rikka_widgets::node_canvas::{
    CanvasEdit, CanvasFrame, CanvasLink, CanvasNode, CanvasPort, CanvasView, Extend, FrameKey,
    Geometry, Hit, Item, MAX_ZOOM, NodeGraph, NodeKey, PortAddr, Selection, Toward, link_curve,
    node_canvas, node_height,
};

use crate::common::{pixel, renderer};

const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);

fn graph() -> NodeGraph {
    let source = CanvasNode::new(NodeKey(1), "Source", Point::new(0.0, 0.0))
        .output(CanvasPort::new("colour", WHITE))
        .output(CanvasPort::new("alpha", WHITE));
    let sink = CanvasNode::new(NodeKey(2), "Sink", Point::new(400.0, 0.0))
        .input(CanvasPort::new("base", WHITE))
        .input(CanvasPort::new("blend", WHITE));
    let below = CanvasNode::new(NodeKey(3), "Below", Point::new(0.0, 300.0))
        .output(CanvasPort::new("value", WHITE));
    NodeGraph {
        nodes: vec![source, sink, below],
        links: vec![CanvasLink::new(
            PortAddr::output(NodeKey(1), 0),
            PortAddr::input(NodeKey(2), 1),
        )],
        frames: vec![
            CanvasFrame::group(FrameKey(7), "Inputs", vec![NodeKey(1)]),
            CanvasFrame::comment(
                FrameKey(8),
                "Note",
                Point::new(700.0, 500.0),
                Size::new(200.0, 120.0),
            ),
        ],
    }
}

#[test]
fn hits_follow_ports_nodes_links_and_frames_at_any_view() {
    let graph = graph();
    let geometry = Geometry::new(&graph);
    let link = graph.links[0];
    let from = geometry.anchor(link.from).unwrap();
    let to = geometry.anchor(link.to).unwrap();
    let middle = link_curve(from, to)[12];
    let views = [
        CanvasView::default(),
        CanvasView {
            pan: Point::new(-120.0, 40.0),
            zoom: 0.5,
        },
        CanvasView {
            pan: Point::new(30.0, -10.0),
            zoom: 2.0,
        },
    ];
    for view in views {
        let at = |graph_point: Point| view.to_local(graph_point);
        let cases = [
            (at(from), Hit::Port(link.from)),
            (at(to + Point::new(2.0, -3.0)), Hit::Port(link.to)),
            (at(Point::new(60.0, 10.0)), Hit::Node(NodeKey(1))),
            (at(middle), Hit::Link(link)),
            (at(Point::new(-10.0, -30.0)), Hit::FrameTitle(FrameKey(7))),
            (at(Point::new(-10.0, 40.0)), Hit::Frame(FrameKey(7))),
            (at(Point::new(895.0, 615.0)), Hit::FrameGrip(FrameKey(8))),
            (at(Point::new(750.0, 510.0)), Hit::FrameTitle(FrameKey(8))),
            (at(Point::new(250.0, 600.0)), Hit::Empty),
        ];
        for (point, expected) in cases {
            assert_eq!(
                geometry.hit(&graph, &view, point),
                expected,
                "at {point:?} under {view:?}"
            );
        }
    }
}

#[test]
fn moved_geometry_carries_ports_and_links() {
    let graph = graph();
    let moved = Geometry::moved(&graph, &[NodeKey(2)], Point::new(50.0, 20.0));
    let still = Geometry::new(&graph);
    let input = PortAddr::input(NodeKey(2), 1);
    assert_eq!(
        moved.anchor(input).unwrap(),
        still.anchor(input).unwrap() + Point::new(50.0, 20.0)
    );
    let (_, points) = moved.link_points().next().unwrap();
    assert_eq!(*points.last().unwrap(), moved.anchor(input).unwrap());
    assert_eq!(
        moved.node_rect(NodeKey(2)).unwrap().size.height,
        node_height(&graph.nodes[1])
    );
}

#[test]
fn zooming_keeps_the_point_under_the_pointer() {
    let view = CanvasView {
        pan: Point::new(13.0, -7.0),
        zoom: 0.8,
    };
    let pointer = Point::new(200.0, 150.0);
    let before = view.to_graph(pointer);
    let zoomed = view.zoomed_at(pointer, 1.7);
    let after = zoomed.to_graph(pointer);
    assert!(before.distance(after) < 1e-3);
    let clamped = view.zoomed_at(pointer, 100.0);
    assert_eq!(clamped.zoom, rikka_widgets::node_canvas::MAX_ZOOM);
}

#[test]
fn framing_fits_everything_inside_the_room() {
    let graph = graph();
    let geometry = Geometry::new(&graph);
    let room = Size::new(640.0, 400.0);
    let view = CanvasView::framing(geometry.bounds().unwrap(), room);
    let bounds = geometry.bounds().unwrap();
    let shown = Rect::from_origin_size(
        view.to_local(bounds.origin),
        Size::new(bounds.width() * view.zoom, bounds.height() * view.zoom),
    );
    assert!(Rect::from_origin_size(Point::ZERO, room).contains_rect(&shown));
}

#[test]
fn selection_follows_clicks_and_boxes() {
    let one = Item::Node(NodeKey(1));
    let two = Item::Node(NodeKey(2));
    let three = Item::Node(NodeKey(3));
    let start = Selection::of([one, two]);
    let shift = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };
    let control = Modifiers {
        control: true,
        ..Modifiers::NONE
    };
    assert_eq!(Extend::from_modifiers(Modifiers::NONE), Extend::Replace);
    assert_eq!(Extend::from_modifiers(shift), Extend::Add);
    assert_eq!(Extend::from_modifiers(control), Extend::Toggle);
    let cases = [
        (start.picked(three, Extend::Replace), vec![three]),
        (start.picked(three, Extend::Add), vec![one, two, three]),
        (start.picked(two, Extend::Toggle), vec![one]),
        (start.pressed(two, Extend::Replace), vec![one, two]),
        (start.pressed(three, Extend::Replace), vec![three]),
        (start.boxed([two, three], Extend::Replace), vec![two, three]),
        (
            start.boxed([two, three], Extend::Add),
            vec![one, two, three],
        ),
        (start.boxed([two, three], Extend::Toggle), vec![one, three]),
    ];
    for (index, (got, want)) in cases.into_iter().enumerate() {
        assert_eq!(got, Selection::of(want), "case {index}");
    }
}

#[test]
fn boxes_and_arrows_find_nodes() {
    let graph = graph();
    let geometry = Geometry::new(&graph);
    let inside: Vec<NodeKey> = geometry
        .nodes_within(Rect::new(-20.0, -20.0, 200.0, 400.0))
        .collect();
    assert_eq!(inside, vec![NodeKey(1), NodeKey(3)]);
    assert_eq!(
        geometry.neighbour(NodeKey(1), Toward::Right),
        Some(NodeKey(2))
    );
    assert_eq!(
        geometry.neighbour(NodeKey(1), Toward::Down),
        Some(NodeKey(3))
    );
    assert_eq!(geometry.neighbour(NodeKey(1), Toward::Left), None);
    assert_eq!(geometry.first(), Some(NodeKey(1)));
}

const ROOM: Size = Size::new(640.0, 480.0);

#[derive(State)]
struct Board {
    graph: Arc<NodeGraph>,
    selection: Selection,
    edits: Vec<CanvasEdit>,
}

fn board(board: &Board) -> Element {
    node_canvas(Arc::clone(&board.graph), board.selection.clone())
        .on_select(|board: &mut Board, selection: Selection| board.selection = selection)
        .on_edit(|board: &mut Board, edit: CanvasEdit| board.edits.push(edit))
        .fill()
        .build()
}

fn mount(graph: &Arc<NodeGraph>) -> (TypedUi<Board>, Scene) {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(
        Board {
            graph: Arc::clone(graph),
            selection: Selection::default(),
            edits: Vec::new(),
        },
        board,
        Theme::dark(),
    );
    ui.set_viewport(ROOM, 1.0);
    ui.render(&mut scene);
    ui.render(&mut scene);
    (ui, scene)
}

fn drag(ui: &mut TypedUi<Board>, scene: &mut Scene, from: Point, to: Point) {
    ui.handle_event(Event::PointerMoved(from));
    ui.handle_event(Event::PointerPressed {
        position: from,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.render(&mut *scene);
    for step in 1..=4 {
        let at = from + (to - from) * (step as f32 / 4.0);
        ui.handle_event(Event::PointerMoved(at));
        ui.render(&mut *scene);
    }
    ui.handle_event(Event::PointerReleased {
        position: to,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.render(&mut *scene);
}

#[test]
fn dragging_links_nodes_and_boxes_reports_edits() {
    let graph = Arc::new(graph());
    let geometry = Geometry::new(&graph);
    let view = CanvasView::framing(geometry.bounds().unwrap(), ROOM);
    let screen = |point: Point| view.to_local(point);
    let (mut ui, mut scene) = mount(&graph);

    let output = PortAddr::output(NodeKey(1), 1);
    let input = PortAddr::input(NodeKey(2), 0);
    drag(
        &mut ui,
        &mut scene,
        screen(geometry.anchor(output).unwrap()),
        screen(geometry.anchor(input).unwrap()),
    );
    let start = graph.nodes[2].position;
    let grip = screen(start + Point::new(40.0, 10.0));
    drag(&mut ui, &mut scene, grip, grip + Point::new(30.0, 15.0));
    let corner = screen(Point::new(-40.0, 280.0));
    drag(
        &mut ui,
        &mut scene,
        corner,
        screen(Point::new(120.0, 330.0)),
    );

    let board = ui.state();
    assert_eq!(board.edits.len(), 2, "{:?}", board.edits);
    assert_eq!(
        board.edits[0],
        CanvasEdit::Connect {
            from: output,
            to: input,
            replacing: None,
        }
    );
    let CanvasEdit::Move(moves) = &board.edits[1] else {
        panic!("the node drag moves: {:?}", board.edits[1]);
    };
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].0, NodeKey(3));
    let moved = moves[0].1 - start;
    assert!(
        (moved.x - 30.0 / view.zoom).abs() < 0.01 && (moved.y - 15.0 / view.zoom).abs() < 0.01,
        "{moved:?}"
    );
    assert_eq!(board.selection, Selection::of([Item::Node(NodeKey(3))]));

    ui.handle_event(Event::KeyPressed {
        key: Key::Delete,
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers: Modifiers::NONE,
    });
    assert_eq!(
        ui.state().edits.last(),
        Some(&CanvasEdit::Delete(Selection::of([Item::Node(NodeKey(3))])))
    );
}

#[test]
fn wheel_away_zooms_in_and_touchpad_scroll_carries_the_canvas() {
    let graph = Arc::new(graph());
    let geometry = Geometry::new(&graph);
    let view = CanvasView::framing(geometry.bounds().unwrap(), ROOM);
    let (mut ui, mut scene) = mount(&graph);

    let start = graph.nodes[2].position;
    let grip = view.to_local(start + Point::new(40.0, 10.0));
    let carried = Point::new(150.0, -80.0);
    ui.handle_event(Event::PointerMoved(grip));
    ui.handle_event(Event::Scroll {
        position: grip,
        delta: ScrollDelta::from_up(1.0),
        modifiers: Modifiers::NONE,
        source: ScrollSource::Wheel,
    });
    ui.handle_event(Event::Scroll {
        position: grip,
        delta: ScrollDelta::from_content(carried),
        modifiers: Modifiers::NONE,
        source: ScrollSource::Continuous(ScrollPhase::Moved),
    });
    ui.render(&mut scene);
    let grip = grip + carried;
    drag(&mut ui, &mut scene, grip, grip + Point::new(36.0, 18.0));

    let board = ui.state();
    let Some(CanvasEdit::Move(moves)) = board.edits.first() else {
        panic!("the node under the carried grip moves: {:?}", board.edits);
    };
    assert_eq!(moves[0].0, NodeKey(3));
    let moved = moves[0].1 - start;
    let unzoomed = Point::new(36.0 / view.zoom, 18.0 / view.zoom);
    assert!(
        (0.0..unzoomed.x - 0.01).contains(&moved.x) && (0.0..unzoomed.y - 0.01).contains(&moved.y),
        "{moved:?}"
    );
}

#[test]
fn list_inputs_fan_their_links_and_dragging_adds_another() {
    let lights = CanvasNode::new(NodeKey(1), "Lights", Point::new(0.0, 0.0))
        .output(CanvasPort::new("key", WHITE))
        .output(CanvasPort::new("fill", WHITE))
        .output(CanvasPort::new("rim", WHITE));
    let world = CanvasNode::new(NodeKey(2), "World", Point::new(400.0, 0.0))
        .input(CanvasPort::new("lights", WHITE).many());
    let list = PortAddr::input(NodeKey(2), 0);
    let graph = Arc::new(NodeGraph {
        nodes: vec![lights, world],
        links: vec![
            CanvasLink::new(PortAddr::output(NodeKey(1), 0), list),
            CanvasLink::new(PortAddr::output(NodeKey(1), 1), list),
        ],
        frames: Vec::new(),
    });
    let geometry = Geometry::new(&graph);
    let ends: Vec<Point> = geometry
        .link_points()
        .map(|(_, points)| *points.last().unwrap())
        .collect();
    let anchor = geometry.anchor(list).unwrap();
    assert_ne!(ends[0], ends[1]);
    assert!(
        ends.iter()
            .all(|end| end.x == anchor.x && end.distance(anchor) < 10.0)
    );

    let view = CanvasView::framing(geometry.bounds().unwrap(), ROOM);
    let (mut ui, mut scene) = mount(&graph);
    let rim = PortAddr::output(NodeKey(1), 2);
    drag(
        &mut ui,
        &mut scene,
        view.to_local(anchor),
        view.to_local(geometry.anchor(rim).unwrap()),
    );
    assert_eq!(
        ui.state().edits,
        [CanvasEdit::Connect {
            from: rim,
            to: list,
            replacing: None,
        }]
    );
}

#[test]
fn wires_stay_drawn_while_their_ends_are_far_outside() {
    let graph = Arc::new(graph());
    let geometry = Geometry::new(&graph);
    let link = graph.links[0];
    let curve = link_curve(
        geometry.anchor(link.from).unwrap(),
        geometry.anchor(link.to).unwrap(),
    );
    let view = CanvasView::framing(geometry.bounds().unwrap(), ROOM);
    let (mut ui, mut scene) = mount(&graph);
    let pointer = view.to_local(curve[4]);
    ui.handle_event(Event::PointerMoved(pointer));
    for _ in 0..12 {
        ui.handle_event(Event::Scroll {
            position: pointer,
            delta: ScrollDelta::from_up(1.0),
            modifiers: Modifiers::NONE,
            source: ScrollSource::Wheel,
        });
        ui.render(&mut scene);
    }
    let end = pointer + (*curve.last().unwrap() - curve[4]) * MAX_ZOOM;
    let carried = Point::new(ROOM.width + 160.0 - end.x, 0.0);
    let probe = pointer + carried;
    assert!(Rect::from_origin_size(Point::ZERO, ROOM).contains(probe));
    ui.handle_event(Event::Scroll {
        position: pointer,
        delta: ScrollDelta::from_content(carried),
        modifiers: Modifiers::NONE,
        source: ScrollSource::Continuous(ScrollPhase::Moved),
    });
    ui.render(&mut scene);

    let mut renderer = renderer();
    let mut target = renderer
        .create_target(ROOM.width as u32, ROOM.height as u32)
        .unwrap();
    renderer.draw(&mut scene, &mut target).unwrap();
    let pixels = renderer.read_pixels(&mut target).unwrap();
    let shade = pixel(
        &pixels,
        ROOM.width as usize,
        probe.x as usize,
        probe.y as usize,
    );
    assert!(shade.0 > 200, "{shade:?} at {probe:?}\n{}", ui.dump());
}
