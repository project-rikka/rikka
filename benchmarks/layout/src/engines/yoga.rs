use std::ffi::{c_int, c_void};

use crate::engine::Engine;
use crate::spec::{Align, BoxStyle, Direction, Extent, Justify, Placed, Spec};

enum YgNode {}

type NodeRef = *mut YgNode;

#[repr(C)]
struct YgSize {
    width: f32,
    height: f32,
}

type MeasureFunc = unsafe extern "C" fn(*const YgNode, f32, c_int, f32, c_int) -> YgSize;

unsafe extern "C" {
    fn YGNodeNew() -> NodeRef;
    fn YGNodeFreeRecursive(node: NodeRef);
    fn YGNodeInsertChild(node: NodeRef, child: NodeRef, index: usize);
    fn YGNodeCalculateLayout(node: NodeRef, width: f32, height: f32, direction: c_int);
    fn YGNodeMarkDirty(node: NodeRef);
    fn YGNodeSetContext(node: NodeRef, context: *mut c_void);
    fn YGNodeGetContext(node: *const YgNode) -> *mut c_void;
    fn YGNodeSetMeasureFunc(node: NodeRef, measure: MeasureFunc);
    fn YGNodeGetChildCount(node: *const YgNode) -> usize;
    fn YGNodeGetChild(node: NodeRef, index: usize) -> NodeRef;
    fn YGNodeStyleSetFlexDirection(node: NodeRef, direction: c_int);
    fn YGNodeStyleSetJustifyContent(node: NodeRef, justify: c_int);
    fn YGNodeStyleSetAlignItems(node: NodeRef, align: c_int);
    fn YGNodeStyleSetAlignContent(node: NodeRef, align: c_int);
    fn YGNodeStyleSetFlexWrap(node: NodeRef, wrap: c_int);
    fn YGNodeStyleSetFlexGrow(node: NodeRef, grow: f32);
    fn YGNodeStyleSetFlexShrink(node: NodeRef, shrink: f32);
    fn YGNodeStyleSetFlexBasis(node: NodeRef, basis: f32);
    fn YGNodeStyleSetFlexBasisAuto(node: NodeRef);
    fn YGNodeStyleSetWidth(node: NodeRef, width: f32);
    fn YGNodeStyleSetWidthAuto(node: NodeRef);
    fn YGNodeStyleSetHeight(node: NodeRef, height: f32);
    fn YGNodeStyleSetHeightAuto(node: NodeRef);
    fn YGNodeStyleSetPadding(node: NodeRef, edge: c_int, padding: f32);
    fn YGNodeStyleSetMargin(node: NodeRef, edge: c_int, margin: f32);
    fn YGNodeStyleSetGap(node: NodeRef, gutter: c_int, gap: f32);
    fn YGNodeLayoutGetLeft(node: *const YgNode) -> f32;
    fn YGNodeLayoutGetTop(node: *const YgNode) -> f32;
    fn YGNodeLayoutGetWidth(node: *const YgNode) -> f32;
    fn YGNodeLayoutGetHeight(node: *const YgNode) -> f32;
}

const FLEX_DIRECTION_COLUMN: c_int = 0;
const FLEX_DIRECTION_ROW: c_int = 2;
const JUSTIFY_FLEX_START: c_int = 0;
const JUSTIFY_CENTER: c_int = 1;
const JUSTIFY_FLEX_END: c_int = 2;
const ALIGN_FLEX_START: c_int = 1;
const ALIGN_CENTER: c_int = 2;
const ALIGN_FLEX_END: c_int = 3;
const ALIGN_STRETCH: c_int = 4;
const WRAP_NO_WRAP: c_int = 0;
const WRAP_WRAP: c_int = 1;
const EDGE_ALL: c_int = 8;
const GUTTER_ALL: c_int = 2;
const DIRECTION_LTR: c_int = 1;
const MEASURE_MODE_EXACTLY: c_int = 1;

unsafe extern "C" fn measure(
    node: *const YgNode,
    width: f32,
    width_mode: c_int,
    height: f32,
    height_mode: c_int,
) -> YgSize {
    let content = unsafe { &*YGNodeGetContext(node).cast::<Extent>() };
    let fit = |content: f32, available: f32, mode: c_int| match mode {
        MEASURE_MODE_EXACTLY => available,
        _ => content,
    };
    YgSize {
        width: fit(content.width, width, width_mode),
        height: fit(content.height, height, height_mode),
    }
}

pub struct Yoga {
    root: NodeRef,
    nodes: Vec<NodeRef>,
    contents: *mut [Extent],
    viewport: Extent,
}

fn apply(node: NodeRef, style: &BoxStyle) {
    unsafe {
        YGNodeStyleSetFlexDirection(
            node,
            match style.direction {
                Direction::Row => FLEX_DIRECTION_ROW,
                Direction::Column => FLEX_DIRECTION_COLUMN,
            },
        );
        YGNodeStyleSetFlexWrap(node, if style.wrap { WRAP_WRAP } else { WRAP_NO_WRAP });
        match style.width {
            Some(width) => YGNodeStyleSetWidth(node, width),
            None => YGNodeStyleSetWidthAuto(node),
        }
        match style.height {
            Some(height) => YGNodeStyleSetHeight(node, height),
            None => YGNodeStyleSetHeightAuto(node),
        }
        YGNodeStyleSetFlexGrow(node, style.grow);
        YGNodeStyleSetFlexShrink(node, style.shrink);
        match style.basis {
            Some(basis) => YGNodeStyleSetFlexBasis(node, basis),
            None => YGNodeStyleSetFlexBasisAuto(node),
        }
        YGNodeStyleSetPadding(node, EDGE_ALL, style.padding);
        YGNodeStyleSetMargin(node, EDGE_ALL, style.margin);
        YGNodeStyleSetGap(node, GUTTER_ALL, style.gap);
        YGNodeStyleSetAlignItems(
            node,
            match style.align {
                Align::Start => ALIGN_FLEX_START,
                Align::Center => ALIGN_CENTER,
                Align::End => ALIGN_FLEX_END,
                Align::Stretch => ALIGN_STRETCH,
            },
        );
        YGNodeStyleSetAlignContent(node, ALIGN_STRETCH);
        YGNodeStyleSetJustifyContent(
            node,
            match style.justify {
                Justify::Start => JUSTIFY_FLEX_START,
                Justify::Center => JUSTIFY_CENTER,
                Justify::End => JUSTIFY_FLEX_END,
            },
        );
    }
}

impl Yoga {
    fn content(&self, node: usize) -> *mut Extent {
        debug_assert!(node < self.contents.len());
        unsafe { self.contents.cast::<Extent>().add(node) }
    }

    fn add(&mut self, spec: &Spec) -> NodeRef {
        let node = unsafe { YGNodeNew() };
        apply(node, &spec.style);
        if let Some(content) = spec.content {
            let stored = self.content(self.nodes.len());
            unsafe {
                stored.write(content);
                YGNodeSetContext(node, stored.cast());
                YGNodeSetMeasureFunc(node, measure);
            }
        }
        self.nodes.push(node);
        for (index, child) in spec.children.iter().enumerate() {
            let child = self.add(child);
            unsafe { YGNodeInsertChild(node, child, index) };
        }
        node
    }

    fn place(node: NodeRef, parent: (f32, f32), placed: &mut Vec<Placed>) {
        let origin = unsafe {
            (
                parent.0 + YGNodeLayoutGetLeft(node),
                parent.1 + YGNodeLayoutGetTop(node),
            )
        };
        placed.push(Placed {
            x: origin.0,
            y: origin.1,
            width: unsafe { YGNodeLayoutGetWidth(node) },
            height: unsafe { YGNodeLayoutGetHeight(node) },
        });
        for index in 0..unsafe { YGNodeGetChildCount(node) } {
            Self::place(unsafe { YGNodeGetChild(node, index) }, origin, placed);
        }
    }
}

impl Engine for Yoga {
    const NAME: &'static str = "yoga";

    fn build(spec: &Spec) -> Self {
        let count = spec.count();
        let mut engine = Self {
            root: std::ptr::null_mut(),
            nodes: Vec::with_capacity(count),
            contents: Box::into_raw(vec![Extent::new(0.0, 0.0); count].into_boxed_slice()),
            viewport: spec.root_extent(),
        };
        engine.root = engine.add(spec);
        engine
    }

    fn compute(&mut self) {
        unsafe {
            YGNodeCalculateLayout(
                self.root,
                self.viewport.width,
                self.viewport.height,
                DIRECTION_LTR,
            );
        }
    }

    fn resize(&mut self, width: f32) {
        self.viewport.width = width;
        unsafe { YGNodeStyleSetWidth(self.root, width) };
    }

    fn resize_leaf(&mut self, node: usize, content: Extent) {
        unsafe {
            self.content(node).write(content);
            YGNodeMarkDirty(self.nodes[node]);
        }
    }

    fn placed(&self) -> Vec<Placed> {
        let mut placed = Vec::with_capacity(self.nodes.len());
        Self::place(self.root, (0.0, 0.0), &mut placed);
        placed
    }
}

impl Drop for Yoga {
    fn drop(&mut self) {
        unsafe {
            YGNodeFreeRecursive(self.root);
            drop(Box::from_raw(self.contents));
        }
    }
}

impl std::fmt::Debug for Yoga {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Yoga")
            .field("nodes", &self.nodes.len())
            .finish()
    }
}
