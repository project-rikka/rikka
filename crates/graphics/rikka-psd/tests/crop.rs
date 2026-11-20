use rikka_psd::{
    ColorMode, Compression, Depth, Document, GridAndGuides, Guide, GuideDirection, Info, Knot,
    Layer, Node, PathData, PathOperation, Point, Rect, ResourceData, Subpath, VectorMask,
    resource_id,
};

use crate::support::{numbered, reds};

fn layered() -> Document {
    let mut document = Document::new(4, 3, Depth::Eight, ColorMode::Rgb);
    document
        .set_composite_rgba(&numbered(4, 3), Compression::Rle)
        .unwrap();
    let mut layer = Layer::new("ink");
    document
        .set_layer_rgba(&mut layer, 1, 1, &numbered(2, 2), Compression::Rle)
        .unwrap();
    let corner = Knot::corner(Point {
        x: 0.5,
        y: 1.0 / 3.0,
    });
    layer.set_info(Info::VectorMask(VectorMask::new(PathData::new(vec![
        Subpath::new(true, PathOperation::Combine, vec![corner]),
    ]))));
    document.layers.push(Node::Layer(layer));
    document
}

#[test]
fn crop_keeps_area_and_moves_layers() {
    let mut document = layered();

    document.crop(Rect::from_size(1, 1, 2, 2)).unwrap();

    assert_eq!((document.width, document.height), (2, 2));
    assert_eq!(reds(&document.composite_rgba().unwrap()), [5, 6, 9, 10]);
    let Node::Layer(layer) = &document.layers[0] else {
        panic!("expected layer");
    };
    assert_eq!(layer.bounds, Rect::from_size(0, 0, 2, 2));
    let Some(Info::VectorMask(mask)) = layer.find(b"vmsk") else {
        panic!("expected vector mask");
    };
    let anchor = mask.path.subpaths().next().unwrap().knots[0].anchor;
    assert!((anchor.x - 0.5).abs() < 1e-9 && anchor.y.abs() < 1e-9);
    let reread = Document::read(&document.to_bytes().unwrap()).unwrap();
    assert_eq!((reread.width, reread.height), (2, 2));
}

#[test]
fn rejects_crop_outside_document() {
    let mut document = layered();

    assert!(document.crop(Rect::from_size(3, 0, 2, 2)).is_err());
    assert!(document.crop(Rect::from_size(0, 0, 0, 2)).is_err());

    assert_eq!(document, layered());
}

#[test]
fn crop_moves_paths_guides_and_drops_thumbnail() {
    let mut document = layered();
    let corner = Knot::corner(Point {
        x: 0.5,
        y: 2.0 / 3.0,
    });
    document.resources.set(
        2000,
        ResourceData::Path(PathData::new(vec![Subpath::new(
            true,
            PathOperation::Combine,
            vec![corner],
        )])),
    );
    document.resources.set(
        resource_id::GRID_AND_GUIDES,
        ResourceData::GridAndGuides(GridAndGuides {
            version: 1,
            grid_horizontal: 576,
            grid_vertical: 576,
            guides: vec![Guide {
                position: 3.0,
                direction: GuideDirection::Vertical,
            }],
        }),
    );
    document
        .resources
        .set(resource_id::THUMBNAIL, ResourceData::Raw(vec![1, 2, 3]));

    document.crop(Rect::from_size(2, 1, 2, 2)).unwrap();

    let (_, path) = document.resources.paths().next().unwrap();
    let anchor = path.subpaths().next().unwrap().knots[0].anchor;
    assert!(anchor.x.abs() < 1e-9 && (anchor.y - 0.5).abs() < 1e-9);
    let Some(ResourceData::GridAndGuides(grid)) =
        document.resources.get(resource_id::GRID_AND_GUIDES)
    else {
        panic!("expected guides");
    };
    assert_eq!(grid.guides[0].position, 1.0);
    assert!(document.resources.get(resource_id::THUMBNAIL).is_none());
}
