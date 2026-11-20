use std::cell::Cell;

use rikka_types::Point;

use crate::paint::{ClipKind, PaintCtx, Quad, SceneClip};
use crate::runtime::handle::serving;
use crate::runtime::instance::Instance;
use crate::runtime::layout::Node;
use crate::runtime::table::ZOOMED;
use crate::runtime::walk::{NodeGeometry, Walk};

impl Walk<'_> {
    pub(super) fn paint(&mut self, node: Node, instance: &mut Instance, drawn: &NodeGeometry) {
        crate::stats::painted();
        self.spare.scratch.clear();
        self.spare.fades.clear();
        self.spare.sections.clear();
        let mut animating = false;
        let mut wake = None;
        let mut wanted_view = false;
        let mut wanted_build = false;
        let mut wanted_prefetch = false;
        let mut grounded = false;
        let mut glass = std::mem::take(&mut instance.outputs.glass);
        glass.clear();
        let mut opaque = std::mem::take(&mut instance.outputs.opaque);
        opaque.clear();
        let mut windows = std::mem::take(&mut instance.outputs.windows);
        windows.clear();
        let mut ctx = PaintCtx {
            bounds: drawn.bounds,
            room: drawn.room,
            content_size: drawn.content,
            viewport: drawn.viewport,
            layouts: self.layouts,
            node,
            theme: self.theme,
            env: &instance.env,
            scale: drawn.scale,
            offset: drawn.offset,
            clip: drawn.base.map(|clip| clip.shape),
            hovered: drawn.hovered,
            focused: drawn.focused,
            focus_within: drawn.focus_within,
            clock: self.clock,
            animating: &mut animating,
            wake: &mut wake,
            motions: &mut instance.motions,
            subtree_opacity: 1.0,
            subtree_scale: None,
            children_clip: None,
            fades: &mut self.spare.fades,
            fade: None,
            out: &mut self.spare.scratch,
            rebuild: &mut wanted_view,
            build: &mut wanted_build,
            prefetch: &mut wanted_prefetch,
            reveal: &mut self.reveal,
            glass: &mut glass,
            opaque: &mut opaque,
            windows: &mut windows,
            pulses: &mut self.out.pulses,
            grounded: &mut grounded,
            clip_kind: ClipKind::Base,
            scene_clip: SceneClip::base(drawn.loose.map(|clip| clip.shape)),
            reads: Cell::new(0),
            instance: instance.id,
            marks: self.marks,
            motion_scope: None,
            topmost: false,
            sections: &mut self.spare.sections,
            section: None,
            section_by: Point::ZERO,
            part: None,
        };
        if let Some(background) = drawn.background {
            ctx.draw_quad(Quad::new(ctx.bounds()).fill(background));
        }
        serving(instance.id, self.marks, || {
            instance.widget.paint(instance.state.as_mut(), &mut ctx);
        });
        instance.told = drawn.bounds.origin;
        if let Some((_, bubble)) = self.bubble.as_ref().filter(|(at, _)| *at == node) {
            crate::runtime::tips::paint(&mut ctx, bubble);
        }
        let subtree_opacity = ctx.subtree_opacity;
        let subtree_scale = ctx.subtree_scale;
        let children_clip = ctx.children_clip;
        let reads = ctx.reads.get();
        let outputs = &mut instance.outputs;
        outputs.subtree_opacity = subtree_opacity;
        outputs.subtree_scale = subtree_scale;
        match subtree_scale {
            Some(_) => self.table.raise(node, ZOOMED),
            None => self.table.lower(node, ZOOMED),
        }
        outputs.children_clip = children_clip;
        outputs.reads = reads;
        outputs.grounded = grounded;
        outputs.glass = glass;
        outputs.opaque = opaque;
        outputs.windows = windows;
        outputs.sections.clone_from(&self.spare.sections);
        outputs.volatile = self.spare.scratch.iter().any(|painted| {
            painted.clip == ClipKind::Volatile
                || painted.scene.volatile()
                || painted.prim.has_cutout()
        });
        outputs.topmost = self.spare.scratch.iter().any(|painted| painted.topmost);
        let hosts = grounded
            || !outputs.glass.is_empty()
            || !outputs.opaque.is_empty()
            || !outputs.windows.is_empty();
        if hosts != instance.hosts {
            instance.hosts = hosts;
            self.out.hosting.push((node, hosts));
        }
        if animating {
            self.out.active.push(instance.id);
        }
        if let Some(at) = wake {
            self.out.wakes.push((at, instance.id));
        }
        self.out.view |= wanted_view;
        instance.asked_build = wanted_build;
        instance.wants_build = wanted_build;
        if wanted_build {
            self.out.builds.push(node);
        }
        if wanted_prefetch {
            self.out.prefetches.push(instance.id);
        }
        self.settle_fades(instance, drawn.layer);
        self.apply(node, instance, drawn);
    }
}
