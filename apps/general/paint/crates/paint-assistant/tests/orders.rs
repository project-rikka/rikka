use std::path::PathBuf;

use paint_assistant::{
    Change, DrawKind, Filling, HistoryOrder, LayerRef, LayerStep, MaskShape, Moving, Order,
    Request, SelectStep, order,
};
use paint_document::{Combine, Modify, Shape};
use rikka_image_edit::{FillRule, Operation, Tool as MarkTool};
use rikka_psd::BlendMode;
use rikka_types::{Color, Point};
use serde_json::json;

fn change(order: Result<Order, String>) -> (String, Change) {
    match order.expect("an order").request {
        Request::Change { name, change } => (name, change),
        other => panic!("a change, not {other:?}"),
    }
}

#[test]
fn adjust_steps_parse_operation_layer_mask() {
    let (name, change) = change(order(
        "adjust",
        json!({
            "name": "空を暖かく",
            "steps": [
                { "op": "white_balance", "temperature": 5200, "layer": "Sky",
                  "mask": { "ellipse": [10, 10, 100, 50], "feather": 20, "invert": true } },
                { "op": "curves", "rgb": [[0, 0], [0.5, 0.55], [1, 1]], "layer": 2,
                  "mask": { "path": "M0 0 H10 V10 Z", "fill_rule": "evenodd" } },
                { "op": "auto_levels" }
            ]
        }),
    ));
    assert_eq!(name, "空を暖かく");
    let Change::Adjust(steps) = change else {
        panic!("an adjustment");
    };
    assert!(matches!(
        steps[0].recipe.steps()[0].operation(),
        Operation::WhiteBalance(_)
    ));
    assert!(matches!(
        steps[2].recipe.steps()[0].operation(),
        Operation::AutoLevels(_)
    ));
    assert_eq!(steps[0].layer, Some(LayerRef::Name("Sky".into())));
    assert_eq!(steps[1].layer, Some(LayerRef::Index(2)));
    let mask = steps[0].mask.as_ref().expect("a mask");
    assert_eq!(mask.shape, MaskShape::Ellipse([10.0, 10.0, 100.0, 50.0]));
    assert!(mask.invert && mask.feather == 20.0);
    assert!(matches!(
        steps[1].mask.as_ref().map(|mask| &mask.shape),
        Some(MaskShape::Path(_, FillRule::EvenOdd))
    ));

    let refused = order(
        "adjust",
        json!({ "name": "x", "steps": [{ "op": "levels" }, { "op": "blurr" }] }),
    )
    .expect_err("no such op");
    assert!(refused.starts_with("step 2 (blurr):"), "{refused}");
    for (step, says) in [
        (
            json!({ "op": "gaussian_blur", "radius": 5000 }),
            "outside 0.1…1000 px",
        ),
        (
            json!({ "op": "gaussian_blur", "sigma": 3 }),
            "`sigma` is no parameter",
        ),
        (
            json!({ "op": "levels", "rgb": { "blak": 3 } }),
            "`rgb.blak` is no parameter",
        ),
        (json!({ "op": "color_lookup" }), "needs `table`"),
        (
            json!({ "op": "invert", "amount": 1 }),
            "takes no parameters",
        ),
    ] {
        let refused =
            order("adjust", json!({ "name": "x", "steps": [step] })).expect_err("a refused step");
        assert!(refused.contains(says), "{refused}");
    }
    let unnamed = order(
        "adjust",
        json!({ "name": " ", "steps": [{ "op": "invert" }] }),
    )
    .expect_err("unnamed");
    assert!(unnamed.contains("name"), "{unnamed}");
    let layer = order(
        "adjust",
        json!({ "name": "x", "steps": [{ "op": "invert", "layer": -1 }] }),
    )
    .expect_err("no such layer reference");
    assert!(layer.contains("index or exact name"), "{layer}");
}

fn drawn(items: serde_json::Value) -> Vec<DrawKind> {
    let (_, change) = change(order(
        "draw",
        json!({ "name": "Draw", "layer": "Ink", "items": items }),
    ));
    let Change::Draw(drawing) = change else {
        panic!("a drawing");
    };
    assert_eq!(drawing.layer, Some(LayerRef::Name("Ink".into())));
    drawing.items
}

fn refusal(item: serde_json::Value) -> String {
    order("draw", json!({ "name": "Draw", "items": [item] })).expect_err("refused")
}

#[test]
fn draw_orders_parse() {
    let items = drawn(json!([
        { "tool": "ellipse", "rect": [10, 20, 30, 40], "color": "#ff0000", "width": 4, "filling": "both", "fill": "#00000080" },
        { "tool": "arrow", "points": [[0, 0], [50, 50]] },
        { "tool": "pen", "points": [[1, 1], [2, 2], [3, 3]] },
        { "tool": "fill", "color": "#ffffff" },
        { "tool": "erase" }
    ]));
    let DrawKind::Mark(ellipse) = &items[0] else {
        panic!("a mark");
    };
    assert_eq!(ellipse.tool, MarkTool::Ellipse);
    assert_eq!(
        ellipse.strokes,
        vec![vec![Point::new(10.0, 20.0), Point::new(40.0, 60.0)]]
    );
    assert_eq!(
        ellipse.filling,
        Some(Filling::Both(Some(Color::rgba8(0, 0, 0, 128))))
    );
    assert_eq!(items[3], DrawKind::Fill(Some(Color::WHITE)));
    assert_eq!(items[4], DrawKind::Erase(None));

    assert!(refusal(json!({ "tool": "line", "points": [[0, 0]] })).contains("exactly 2"));
    let stray = refusal(json!({ "tool": "pen", "points": [[0, 0]], "filling": "fill" }));
    assert!(
        stray.contains("`filling` is not for pen; it takes points, color, width"),
        "{stray}"
    );
    let unknown = refusal(json!({ "tool": "spray", "points": [[0, 0]] }));
    assert!(
        unknown.contains("pen, highlighter") && unknown.contains("bucket"),
        "{unknown}"
    );
}

fn selected(steps: serde_json::Value) -> Vec<SelectStep> {
    let (_, change) = change(order("select", json!({ "name": "Select", "steps": steps })));
    let Change::Select(steps) = change else {
        panic!("a selection");
    };
    steps
}

#[test]
fn selection_steps_parse() {
    let steps = selected(json!([
        { "op": "ellipse", "rect": [10, 10, 40, 20], "antialias": false },
        { "op": "rectangle", "rect": [0, 0, 5, 5], "combine": "add" },
        { "op": "polygon", "points": [[0, 0], [10, 0], [5, 8]], "combine": "subtract" },
        { "op": "path", "d": "M0 0 H10 V10 Z", "fill_rule": "evenodd" },
        { "op": "wand", "at": [3, 4], "tolerance": 10, "mode": "global", "combine": "intersect" },
        { "op": "layer", "layer": "Ink" },
        { "op": "feather", "by": 12.5 },
        { "op": "expand", "by": 3 },
        { "op": "move", "dx": -4, "dy": 2 },
        { "op": "invert" }
    ]));
    assert_eq!(
        steps[0],
        SelectStep::Shape {
            shape: Shape::ellipse(10.0, 10.0, 40.0, 20.0).anti_aliased(false),
            combine: Combine::Replace
        }
    );
    assert!(matches!(
        steps[1],
        SelectStep::Shape {
            combine: Combine::Union,
            ..
        }
    ));
    assert!(matches!(
        steps[2],
        SelectStep::Shape {
            combine: Combine::Exclude,
            ..
        }
    ));
    let SelectStep::Wand { flood, combine } = &steps[4] else {
        panic!("a wand");
    };
    assert_eq!(
        (flood.at, flood.tolerance, *combine),
        ((3.0, 4.0), Some(0.1), Combine::Intersect)
    );
    assert_eq!(
        steps[5],
        SelectStep::Layer {
            layer: Some(LayerRef::Name("Ink".into())),
            combine: Combine::Replace
        }
    );
    assert_eq!(steps[6], SelectStep::Modify(Modify::Feather(12.5)));
    assert_eq!(steps[7], SelectStep::Modify(Modify::Expand(3)));
    assert_eq!(steps[8], SelectStep::Move { dx: -4, dy: 2 });
    assert_eq!(steps[9], SelectStep::Invert);

    let refused = order(
        "select",
        json!({ "name": "x", "steps": [{ "op": "feather" }] }),
    )
    .expect_err("no amount");
    assert!(refused.contains("needs `by`"), "{refused}");
    let stray = order(
        "select",
        json!({ "name": "x", "steps": [{ "op": "all", "rect": [0, 0, 1, 1] }] }),
    )
    .expect_err("all takes nothing");
    assert!(stray.contains("it takes nothing more"), "{stray}");
}

fn layered(steps: serde_json::Value) -> Vec<LayerStep> {
    let (_, change) = change(order("layers", json!({ "name": "Layers", "steps": steps })));
    let Change::Layers(steps) = change else {
        panic!("layer steps");
    };
    steps
}

#[test]
fn layer_steps_resolve_index_or_name() {
    let steps = layered(json!([
        { "op": "new", "name": "Sky", "above": 0 },
        { "op": "set", "layer": "Sky", "opacity": 60 },
        { "op": "set", "layer": "Sky", "blend": "soft_light" },
        { "op": "move", "layer": "Sky", "below": "Text" },
        { "op": "move", "by": 2 },
        { "op": "set", "layer": 3, "name": "Clouds", "visible": false },
        { "op": "duplicate" },
        { "op": "merge_down", "layer": "Clouds" },
        { "op": "flatten" }
    ]));
    assert_eq!(
        steps[0],
        LayerStep::New {
            name: "Sky".into(),
            above: Some(LayerRef::Index(0))
        }
    );
    let LayerStep::Restyle { layer, look } = &steps[1] else {
        panic!("a restyle");
    };
    assert_eq!(
        (layer, look.opacity),
        (&Some(LayerRef::Name("Sky".into())), Some(0.6))
    );
    let LayerStep::Restyle { look, .. } = &steps[2] else {
        panic!("a restyle");
    };
    assert_eq!(look.blend, Some(BlendMode::SoftLight));
    assert_eq!(
        steps[3],
        LayerStep::Move {
            layer: Some(LayerRef::Name("Sky".into())),
            to: Moving::Below(LayerRef::Name("Text".into()))
        }
    );
    assert_eq!(
        steps[4],
        LayerStep::Move {
            layer: None,
            to: Moving::By(2)
        }
    );
    assert_eq!(steps[8], LayerStep::Flatten);

    let blend = order(
        "layers",
        json!({ "name": "x", "steps": [{ "op": "set", "blend": "glow" }] }),
    )
    .expect_err("no such blend");
    assert!(
        blend.contains("multiply") && blend.contains("luminosity"),
        "{blend}"
    );
    let twice = order(
        "layers",
        json!({ "name": "x", "steps": [{ "op": "move", "above": 0, "below": 1 }] }),
    )
    .expect_err("one place");
    assert!(twice.contains("`by`"), "{twice}");
    assert!(order("layers", json!({ "name": "x", "steps": [{ "op": "set" }] })).is_err());
    assert!(order("layers", json!({ "name": "x", "steps": [{ "op": "new" }] })).is_err());
}

#[test]
fn history_and_file_orders_parse() {
    let Request::History(undo) = order("history", json!({ "action": "undo", "steps": 3 }))
        .expect("an order")
        .request
    else {
        panic!("history");
    };
    assert_eq!(undo, HistoryOrder::Undo(3));
    assert!(order("history", json!({ "action": "revisit" })).is_err());
    let home = rikka_dirs::home().expect("every test runs with a home folder");
    let inside = home.join("Pictures/cat.jpg");
    let Request::Save(saving) = order("save", json!({ "path": inside }))
        .expect("a save inside the home folder")
        .request
    else {
        panic!("a save");
    };
    assert_eq!(saving.path, Some(inside));
    for refused in [
        PathBuf::from("Pictures/cat.jpg"),
        PathBuf::from("/etc/cat.png"),
        home.join("../other/cat.png"),
        home.join("cat.txt"),
        home.to_path_buf(),
    ] {
        assert!(
            order("save", json!({ "path": refused })).is_err(),
            "{}",
            refused.display()
        );
    }
    assert!(order("save", json!({ "quality": 0 })).is_err());
    assert!(order("open", json!({ "path": "relative.png" })).is_err());
}
