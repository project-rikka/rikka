use rikka_psd::{ColorMode, Depth, Document, Error, Group, Layer, Node, NodePath};

fn nested() -> Document {
    let mut document = Document::new(4, 4, Depth::Eight, ColorMode::Rgb);
    document.layers = vec![
        Layer::new("a").into(),
        Group::new("outer")
            .with_children(vec![
                Layer::new("b").into(),
                Group::new("inner")
                    .with_children(vec![Layer::new("c").into()])
                    .into(),
                Layer::new("d").into(),
            ])
            .into(),
        Layer::new("e").into(),
    ];
    document
}

#[test]
fn preorder_positions_and_paths_agree() {
    let document = nested();
    let outer = NodePath::root(1);
    let inner = outer.child(1);
    let expected = [
        ("a", NodePath::root(0)),
        ("outer", outer.clone()),
        ("b", outer.child(0)),
        ("inner", inner.clone()),
        ("c", inner.child(0)),
        ("d", outer.child(2)),
        ("e", NodePath::root(2)),
    ];

    let names: Vec<&str> = document.preorder().map(Node::name).collect();
    assert_eq!(names, expected.each_ref().map(|(name, _)| *name));
    for (position, (name, path)) in expected.iter().enumerate() {
        assert_eq!(document.preorder_path(position).as_ref(), Some(path));
        assert_eq!(document.preorder_index(path), Some(position));
        assert_eq!(document.node(path).map(Node::name), Some(*name));
    }
    assert_eq!(document.preorder_path(expected.len()), None);
    assert_eq!(document.preorder_index(&NodePath::root(3)), None);
    assert_eq!(document.node(&NodePath::root(0).child(0)), None);
}

#[test]
fn moved_node_lands_where_its_path_points_after_removal() {
    let mut document = nested();
    let from = NodePath::root(0);
    let target = NodePath::root(1).child(1).child(1);

    let moved = document.remove_node(&from).unwrap();
    let target = target.after_removal(&from);
    document.insert_node(&target, moved).unwrap();

    let names: Vec<&str> = document.preorder().map(Node::name).collect();
    assert_eq!(names, ["outer", "b", "inner", "c", "a", "d", "e"]);
    assert_eq!(target, NodePath::root(0).child(1).child(1));
    assert!(NodePath::root(0).holds(&target));
    assert_eq!(
        document.insert_node(&NodePath::root(1).child(0), Layer::new("x").into()),
        Err(Error::NoPlace)
    );
}
