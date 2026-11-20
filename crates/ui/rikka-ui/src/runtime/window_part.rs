use rikka_types::Point;

use crate::runtime::route::HitRoute;
use crate::runtime::tree::UiTree;
use crate::window::WindowPart;

impl UiTree {
    pub(crate) fn window_part_at(&self, point: Point) -> Option<WindowPart> {
        let root = self.root?;
        let mut route = HitRoute::default();
        if !route.hit(
            &self.table,
            &self.layouts,
            root,
            Some(point),
            self.has_anchors(),
        ) {
            return None;
        }
        let path = route.path();
        let (&deepest, lineage) = path.split_last()?;
        let mut seen = self.table.seen(&self.layouts, *path.first()?);
        for &node in lineage {
            seen = seen.within(&self.table, node);
        }
        self.table
            .get(deepest)
            .widget
            .window_part(seen.bounds(&self.table, deepest), point)
    }
}
