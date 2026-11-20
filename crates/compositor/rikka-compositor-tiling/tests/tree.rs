use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_tiling::{Edges, Gaps, Tree};

const GAPS: Gaps = Gaps {
    inner: 8,
    outer: 12,
};

const TIGHT: Gaps = Gaps { inner: 0, outer: 0 };

fn area() -> Rect<i32, Logical> {
    Rect::new((0, 40).into(), (1920, 1040).into())
}

fn grown(count: u32) -> Tree<u32> {
    let mut tree = Tree::default();
    for window in 0..count {
        tree.insert(area(), window, None);
    }
    tree
}

fn tile_of(tree: &Tree<u32>, gaps: Gaps, window: u32) -> Rect<i32, Logical> {
    tree.tiles(area(), gaps)
        .into_iter()
        .find(|(tiled, _)| *tiled == window)
        .map(|(_, tile)| tile)
        .expect("window is tiled")
}

#[test]
fn tiles_fill_the_area_without_overlap() {
    for count in 1..=7 {
        let tree = grown(count);
        let tiles = tree.tiles(area(), TIGHT);
        assert_eq!(tiles.len(), count as usize);
        let covered: i64 = tiles
            .iter()
            .map(|(_, tile)| i64::from(tile.size.w) * i64::from(tile.size.h))
            .sum();
        assert_eq!(covered, 1920 * 1040, "{count} windows");
        for (index, (_, tile)) in tiles.iter().enumerate() {
            assert!(area().contains_rect(*tile), "{tile:?}");
            for (_, other) in &tiles[index + 1..] {
                assert!(!tile.overlaps(*other), "{tile:?} over {other:?}");
            }
        }
    }
}

#[test]
fn gaps_separate_tiles_and_screen_edges() {
    let tiles = grown(4).tiles(area(), GAPS);
    let inside = Rect::new((12, 52).into(), (1896, 1016).into());
    for (index, (_, tile)) in tiles.iter().enumerate() {
        assert!(inside.contains_rect(*tile), "{tile:?}");
        for (_, other) in &tiles[index + 1..] {
            let apart_x = (tile.loc.x - (other.loc.x + other.size.w))
                .max(other.loc.x - (tile.loc.x + tile.size.w));
            let apart_y = (tile.loc.y - (other.loc.y + other.size.h))
                .max(other.loc.y - (tile.loc.y + tile.size.h));
            assert!(apart_x.max(apart_y) >= GAPS.inner, "{tile:?} and {other:?}");
        }
    }
    assert_eq!(tiles[0].1.loc, Point::from((12, 52)));
}

#[test]
fn arrival_is_where_the_window_lands() {
    let mut tree = Tree::default();
    for window in 0..6 {
        let next_to = (window == 4).then_some(&1);
        let arrival = tree.arrival(area(), GAPS, next_to);
        tree.insert(area(), window, next_to);
        assert_eq!(tile_of(&tree, GAPS, window), arrival, "window {window}");
    }
}

#[test]
fn removed_window_leaves_its_space_to_the_rest() {
    let mut tree = grown(5);
    assert!(tree.remove(&2));
    assert!(!tree.remove(&2));
    assert_eq!(tree.windows(), vec![0, 1, 3, 4]);
    for window in [0, 1, 3] {
        assert!(tree.remove(&window));
    }
    assert_eq!(tile_of(&tree, TIGHT, 4), area());
    assert!(tree.remove(&4));
    assert!(tree.is_empty());
}

#[test]
fn dragged_edge_moves_the_shared_split() {
    let mut tree = grown(2);
    let (left, right) = (tile_of(&tree, TIGHT, 0), tile_of(&tree, TIGHT, 1));
    assert_eq!((left.size.w, right.size.w), (960, 960));

    assert!(tree.resize(area(), TIGHT, &0, Edges::RIGHT, Point::from((96.0, 0.0))));
    assert_eq!(tile_of(&tree, TIGHT, 0).size.w, 1056);
    assert_eq!(tile_of(&tree, TIGHT, 1).size.w, 864);

    assert!(!tree.resize(area(), TIGHT, &0, Edges::LEFT, Point::from((96.0, 0.0))));
    assert!(!tree.resize(area(), TIGHT, &0, Edges::BOTTOM, Point::from((0.0, 96.0))));

    assert!(tree.resize(area(), TIGHT, &1, Edges::LEFT, Point::from((9000.0, 0.0))));
    assert_eq!(tile_of(&tree, TIGHT, 1).size.w, 96);
    assert!(!tree.resize(area(), TIGHT, &1, Edges::LEFT, Point::from((50.0, 0.0))));
}

#[test]
fn swapped_windows_trade_tiles() {
    let mut tree = grown(3);
    let (first, last) = (tile_of(&tree, GAPS, 0), tile_of(&tree, GAPS, 2));
    assert!(tree.swap(&0, &2));
    assert_eq!(tile_of(&tree, GAPS, 0), last);
    assert_eq!(tile_of(&tree, GAPS, 2), first);
    assert!(!tree.swap(&0, &0));
    assert!(!tree.swap(&0, &9));
}
