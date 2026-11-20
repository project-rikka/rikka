use std::sync::Arc;

use rikka_vector::Transform;

use crate::parse::dom::{NodeId, Tag};
use crate::parse::values::{aspect_ratio, reference};
use crate::tree::Group;
use crate::tree::build::Builder;
use crate::tree::filters::{Filter, ImageContent, MAX_PRIMITIVES, filter};
use crate::tree::style::{Axis, Context};

impl Builder<'_> {
    pub(super) fn filter_list(&mut self, value: &str) -> Option<Vec<Arc<Filter>>> {
        let mut found = Vec::new();
        for entry in value.split("url(").skip(1) {
            let id = entry
                .split(')')
                .next()
                .map(|inner| inner.trim().trim_matches(['"', '\'']))
                .and_then(|inner| inner.strip_prefix('#'))?;
            let node = self.dom.find(id, &[Tag::Filter])?;
            let built = self.resource(
                node,
                |builder| &mut builder.filters,
                |builder| {
                    let mut ancestors = builder.ancestry(node);
                    ancestors.pop();
                    let (dom, viewport) = (builder.dom, builder.viewport);
                    filter(dom, node, &ancestors, viewport, &mut |child| {
                        builder.filter_image(child)
                    })
                    .map(Arc::new)
                },
            )?;
            found.push(built);
        }
        let primitives: usize = found.iter().map(|filter| filter.primitives.len()).sum();
        (primitives <= MAX_PRIMITIVES).then_some(found)
    }

    fn filter_image(&mut self, node: NodeId) -> Option<ImageContent> {
        let href = self.dom.attribute(node, "href")?;
        let ancestry = self.ancestry(node);
        let context = Context {
            dom: self.dom,
            chain: &ancestry,
            viewport: self.viewport,
        };
        if href.trim_start().starts_with('#') {
            let target = reference(href).and_then(|id| self.dom.by_id(id))?;
            if ancestry.contains(&target) {
                return None;
            }
            let offset = Transform::translate(
                context.coordinate("x", Axis::Horizontal).unwrap_or(0.0),
                context.coordinate("y", Axis::Vertical).unwrap_or(0.0),
            );
            let mut parents = self.ancestry(target);
            parents.pop();
            let chain = std::mem::replace(&mut self.chain, parents);
            let mut children = Vec::new();
            self.element(target, &mut children);
            self.chain = chain;
            return Some(ImageContent::Element(Group {
                transform: offset,
                children,
                ..Group::default()
            }));
        }
        let aspect = context
            .attribute("preserveAspectRatio")
            .map(aspect_ratio)
            .unwrap_or_default();
        let smooth = !matches!(
            context.property("image-rendering").as_deref(),
            Some("optimizeSpeed" | "pixelated" | "crisp-edges")
        );
        let content = self.picture(node)?;
        Some(ImageContent::Picture {
            content,
            aspect,
            smooth,
        })
    }
}
