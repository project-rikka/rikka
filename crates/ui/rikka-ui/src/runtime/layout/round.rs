use rikka_types::{Rect, Size};

use super::edges::{X, Y};
use super::{Layouts, Node, PLACED};

fn round(value: f32) -> f32 {
    (value + 0.5).floor()
}

impl Layouts {
    pub(super) fn round(&mut self, node: Node, parent: [f32; 2]) {
        let at = node.index();
        let local = self.at[at];
        let world = [parent[X] + local[X], parent[Y] + local[Y]];
        let origin = self.rect[at].origin;
        let unchanged = origin.x == round(local[X]) && origin.y == round(local[Y]);
        if self.flags[at] & PLACED == 0 && self.world[at] == world && unchanged {
            return;
        }
        self.flags[at] &= !PLACED;
        self.world[at] = world;
        let [width, height] = self.size[at];
        let [wide, tall] = self.content[at];
        let rect = Rect::new(
            round(local[X]),
            round(local[Y]),
            round(world[X] + width) - round(world[X]),
            round(world[Y] + height) - round(world[Y]),
        );
        let shown = Size::new(
            round(world[X] + wide) - round(world[X]),
            round(world[Y] + tall) - round(world[Y]),
        );
        if self.rect[at] != rect || self.shown[at] != shown {
            self.rect[at] = rect;
            self.shown[at] = shown;
            self.moved.push(node);
        }
        for index in 0..self.children[at].len() {
            let child = self.children[at][index];
            self.round(child, world);
        }
    }
}
