use std::fmt::{self, Write};

use rikka_types::{Point, Rect, RoundedRect, Size};

use crate::paint::Primitive;
use crate::runtime::layout::Node;
use crate::runtime::table::{CULLED, Seen};
use crate::runtime::tree::UiTree;

impl UiTree {
    pub(crate) fn dump(&self) -> String {
        let mut out = String::new();
        if let Some(root) = self.root {
            self.dump_node(&mut out, root, 0, Seen::WINDOW, Point::ZERO, self.scale)
                .expect("writing to a String cannot fail");
        }
        out
    }

    fn dump_node(
        &self,
        out: &mut impl Write,
        node: Node,
        depth: usize,
        seen: Seen,
        scrolled: Point,
        scale: f32,
    ) -> fmt::Result {
        let indent = "  ".repeat(depth);
        let instance = self.table.get(node);
        let bounds = match instance.last_context {
            Some(_) => seen.bounds(&self.table, node),
            None => Rect::ZERO,
        };
        write!(
            out,
            "{indent}{} {} content {}",
            instance.widget.debug_name(),
            rect(bounds),
            size(instance.content_size),
        )?;
        let culled = self.table.has(node, CULLED);
        match culled {
            true => out.write_str(" culled")?,
            false => {
                if let Some(clip) = seen.clip(&self.table, node) {
                    write!(out, " clip {}", rect(clip))?;
                }
            }
        }
        out.write_char('\n')?;
        let drawn = instance.drawn;
        let scrolled = match drawn.is_some_and(|drawn| drawn.scale != scale) {
            true => Point::ZERO,
            false => scrolled,
        };
        if !culled {
            let displaced = drawn.map_or(Point::ZERO, |drawn| {
                (bounds.origin - drawn.bounds.origin) * drawn.scale
            });
            for slot in instance.prims.iter() {
                let mut translated = slot.painted();
                translated.translate(scrolled - displaced);
                writeln!(out, "{indent}  · {}", primitive(&translated))?;
            }
        }
        let mut inner_scale = drawn.map_or(scale, |drawn| drawn.scale);
        let mut inner_scrolled = scrolled;
        if let Some((factor, _)) = instance.outputs.subtree_scale {
            inner_scale *= factor;
            inner_scrolled = Point::ZERO;
        }
        if instance.layout.scrolls.is_some() {
            inner_scrolled += self.table.inner(node) * inner_scale;
        }
        let inside = seen.within(&self.table, node);
        for &child in self.layouts.children(node) {
            self.dump_node(out, child, depth + 1, inside, inner_scrolled, inner_scale)?;
        }
        Ok(())
    }
}

fn primitive(prim: &Primitive) -> String {
    match prim {
        Primitive::Quad { rect: at, clip, .. } => format!("quad {}{}", rect(*at), clipped(*clip)),
        Primitive::Shadow { rect: at, clip, .. } => {
            format!("shadow {}{}", rect(*at), clipped(*clip))
        }
        Primitive::Arc {
            center,
            radius,
            clip,
            ..
        } => format!(
            "arc ({:.2}, {:.2}) r {:.2}{}",
            center.x,
            center.y,
            radius,
            clipped(*clip)
        ),
        Primitive::Text {
            rect: at,
            run,
            clip,
            ..
        } => format!("text {} {:?}{}", rect(*at), run.content, clipped(*clip)),
        Primitive::Icon { rect: at, clip, .. } => format!("icon {}{}", rect(*at), clipped(*clip)),
        Primitive::Image { rect: at, clip, .. } => {
            format!("image {}{}", rect(*at), clipped(*clip))
        }
        Primitive::Segment {
            rect: at,
            rotation,
            clip,
            ..
        } => format!(
            "segment {} turned {rotation:.3}{}",
            rect(*at),
            clipped(*clip)
        ),
    }
}

fn clipped(clip: Option<RoundedRect>) -> String {
    clip.map(|clip| format!(" clip {}", rounded(clip)))
        .unwrap_or_default()
}

fn rounded(clip: RoundedRect) -> String {
    rect(clip.rect)
}

fn rect(rect: Rect) -> String {
    format!(
        "[{:.2} {:.2} {:.2}x{:.2}]",
        rect.x(),
        rect.y(),
        rect.width(),
        rect.height()
    )
}

fn size(size: Size) -> String {
    format!("{:.2}x{:.2}", size.width, size.height)
}
