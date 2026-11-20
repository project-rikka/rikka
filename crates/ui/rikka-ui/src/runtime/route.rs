use rikka_types::Point;

use crate::runtime::layout::{Layouts, Node};
use crate::runtime::table::{CULLED, Seen, Table};
use crate::runtime::zoom::Zoom;
use crate::view::Behavior;

#[derive(Debug, Default)]
pub(crate) struct HitRoute {
    nodes: Vec<Node>,
    ids: Vec<u64>,
    anchored: Vec<Node>,
}

impl HitRoute {
    pub(crate) fn path(&self) -> &[Node] {
        &self.nodes
    }

    pub(crate) fn ids(&self) -> &[u64] {
        &self.ids
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    fn clear(&mut self) {
        self.nodes.clear();
        self.ids.clear();
    }

    pub(crate) fn shared_depth(&self, other: &Self) -> usize {
        self.ids
            .iter()
            .zip(&other.ids)
            .take_while(|(left, right)| left == right)
            .count()
    }

    pub(crate) fn hit(
        &mut self,
        table: &Table,
        layouts: &Layouts,
        root: Node,
        point: Option<Point>,
        anchored: bool,
    ) -> bool {
        self.clear();
        let Some(point) = point else {
            return false;
        };
        if anchored && self.hit_anchored(table, layouts, root, point) {
            return true;
        }
        self.clear();
        self.descend(table, layouts, root, Seen::WINDOW, point)
    }

    fn hit_anchored(&mut self, table: &Table, layouts: &Layouts, root: Node, point: Point) -> bool {
        let mut anchored = std::mem::take(&mut self.anchored);
        anchored.clear();
        gather(table, layouts, root, &mut anchored);
        let found = anchored.iter().rev().any(|&node| {
            self.clear();
            if let Some(parent) = layouts.parent(node) {
                layouts.lineage(parent, &mut self.nodes);
            }
            self.ids
                .extend(self.nodes.iter().map(|&step| table.get(step).id));
            let within = self.nodes.iter().fold(point, |at, &step| {
                Zoom::of(table, step).map_or(at, |zoom| zoom.unzoom(at))
            });
            self.descend(table, layouts, node, table.seen(layouts, node), within)
        });
        self.anchored = anchored;
        found
    }

    fn descend(
        &mut self,
        table: &Table,
        layouts: &Layouts,
        node: Node,
        seen: Seen,
        point: Point,
    ) -> bool {
        let zoom = Zoom::of(table, node);
        let reached = match zoom {
            Some(_) => !table.has(node, CULLED),
            None => seen.reaches(table, node, point),
        };
        if !reached || table.behavior(node).has(Behavior::INERT) {
            return false;
        }
        self.nodes.push(node);
        self.ids.push(table.get(node).id);
        let within = zoom.map_or(point, |zoom| zoom.unzoom(point));
        let inside = seen.within(table, node);
        for &child in layouts.children(node).iter().rev() {
            if self.descend(table, layouts, child, inside, within) {
                return true;
            }
        }
        if seen.contains(table, node, point) {
            return true;
        }
        self.nodes.pop();
        self.ids.pop();
        false
    }
}

fn gather(table: &Table, layouts: &Layouts, node: Node, anchored: &mut Vec<Node>) {
    for &child in layouts.children(node) {
        let instance = table.get(child);
        if table.has(child, CULLED) || !instance.paints_offscreen {
            continue;
        }
        if instance.anchor.is_some() {
            anchored.push(child);
        }
        gather(table, layouts, child, anchored);
    }
}
