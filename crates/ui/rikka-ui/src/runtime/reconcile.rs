use std::any::TypeId;
use std::rc::Rc;
use std::sync::Arc;

use rikka_renderer::Scene;
use rikka_types::Point;
use rustc_hash::FxHashSet;

use crate::ambient::ThemeHandle;
use crate::motion::Passage;
use crate::runtime::graveyard::Graveyard;
use crate::runtime::handle::{Marks, serving};
use crate::runtime::holding::{ComponentSlot, Outer};
use crate::runtime::images::Images;
use crate::runtime::instance::{Exit, Instance};
use crate::runtime::layout::{Fix, Layouts, Node};
use crate::runtime::readers::{Producer, Readers};
use crate::runtime::scene_sync::forget_prims;
use crate::runtime::scratch::{Recycled, Scratch};
use crate::runtime::table::{CULLED, PAINT, Table};
use crate::runtime::walk::Outputs;
use crate::style::{Layout, Length, Style};
use crate::view::observe::{log, reads};
use crate::view::{Anchor, AnyWidget, Behavior, Element, Env, Scoped, State, StateAccess, Store};

const MATCH_AHEAD: usize = 4;

const MATCHED_HEAD: usize = std::mem::offset_of!(Instance, leaving) + size_of::<Option<Exit>>();

pub(crate) struct ReconcileCx<'a> {
    pub(crate) layouts: &'a mut Layouts,
    pub(crate) table: &'a mut Table,
    pub(crate) scene: &'a mut Scene,
    pub(crate) images: &'a mut Images,
    pub(crate) marks: &'a Arc<Marks>,
    pub(crate) scrollers: &'a mut FxHashSet<Node>,
    pub(crate) arrangers: &'a mut FxHashSet<Node>,
    pub(crate) hosting: &'a mut FxHashSet<Node>,
    pub(crate) imaged: &'a mut FxHashSet<Node>,
    pub(crate) theme: &'a ThemeHandle,
    pub(crate) epoch: u64,
    pub(crate) has_anchors: &'a mut bool,
    pub(crate) autofocus: &'a mut Vec<u64>,
    pub(crate) claimed: &'a mut Option<u64>,
    pub(crate) modals: &'a mut Vec<u64>,
    pub(crate) arrivals: bool,
    pub(crate) state: &'a mut dyn State,
    pub(crate) readers: &'a mut Readers,
    pub(crate) chain: Vec<Rc<Scoped>>,
    pub(crate) producer: Producer,
    pub(crate) attributed: Vec<(Producer, Vec<u64>)>,
    pub(crate) everything: bool,
    pub(crate) retained: bool,
    pub(crate) taken_at: u64,
    pub(crate) graveyard: &'a mut Graveyard,
}

impl ReconcileCx<'_> {
    fn descend<R>(
        &mut self,
        scopes: &[Rc<Scoped>],
        producer: Producer,
        body: impl FnOnce(&mut ReconcileCx<'_>) -> R,
    ) -> R {
        let depth = self.chain.len();
        self.chain.extend(scopes.iter().cloned());
        let previous_producer = std::mem::replace(&mut self.producer, producer);
        let result = body(self);
        self.producer = previous_producer;
        self.chain.truncate(depth);
        result
    }

    fn attribute(&mut self, producer: Producer, keys: Vec<u64>) {
        if !keys.is_empty() {
            self.attributed.push((producer, keys));
        }
    }
}

pub(crate) fn last_of(layouts: &Layouts, node: Node) -> Node {
    let mut last = node;
    while let Some(&child) = layouts.children(last).last() {
        last = child;
    }
    last
}

pub(crate) fn singular(behavior: Behavior, anchor: Option<Anchor>) -> bool {
    anchor.is_some()
        || behavior.has(Behavior::MODAL)
        || behavior.has(Behavior::AUTOFOCUS)
        || behavior.has(Behavior::TAKES_FOCUS)
}

fn producing(instance: &Instance, inherited: Producer) -> Producer {
    match (instance.builds, instance.component.is_some()) {
        (true, _) => Producer::Build(instance.id),
        (false, true) => Producer::View(instance.id),
        (false, false) => inherited,
    }
}

pub(crate) fn rebuild_walk(cx: &mut ReconcileCx<'_>, node: Node) -> bool {
    if !cx.table.get(node).wants_build {
        return false;
    }
    let mut instance = cx.table.take(node);
    instance.wants_build = false;
    if instance.asked_build {
        let was = instance.singular;
        rebuild(cx, &mut instance);
        let singular = was || instance.singular;
        cx.table.put(node, instance);
        return singular;
    }
    if instance.widget.freezes_children() {
        cx.table.put(node, instance);
        return false;
    }
    let children = cx.layouts.children(node).to_vec();
    let producer = producing(&instance, cx.producer);
    let scopes = instance.scopes.clone();
    let (next, singular) = cx.descend(&scopes, producer, |cx| {
        let mut singular = false;
        let next: Vec<Node> = children
            .iter()
            .map(|&child| match current_element(cx, child) {
                Some((element, env)) => {
                    singular = true;
                    reconcile(cx, child, element, &env)
                }
                None => {
                    singular |= rebuild_walk(cx, child);
                    child
                }
            })
            .collect();
        (next, singular)
    });
    settle_children(cx, node, &children, &next);
    for child in &next {
        instance.wants_build |= cx.table.get(*child).wants_build;
    }
    settle_aggregates(cx, &mut instance);
    cx.table.put(node, instance);
    singular
}

pub(crate) fn current_element(cx: &mut ReconcileCx<'_>, node: Node) -> Option<(Element, Env)> {
    cx.table
        .get_mut(node)
        .component
        .as_mut()
        .filter(|component| component.dirty)?
        .current_element()
}

fn settle_children(
    cx: &mut ReconcileCx<'_>,
    parent: Node,
    previous_producer: &[Node],
    next: &[Node],
) {
    let detached = next
        .iter()
        .any(|&child| cx.layouts.parent(child) != Some(parent));
    if detached || next != previous_producer {
        replace_children(cx, parent, next);
    }
}

fn replace_children(cx: &mut ReconcileCx<'_>, parent: Node, children: &[Node]) {
    cx.layouts.set_children(parent, children);
    cx.table.islands.mark(parent);
    order_children(cx, parent, children);
}

fn rebuild(cx: &mut ReconcileCx<'_>, instance: &mut Instance) {
    instance.asked_build = false;
    instance.wants_build = false;
    cx.table.mark(cx.layouts, instance.node, PAINT);
    relayout(cx, instance);
    remeasure(cx.layouts, cx.theme, instance);
    if instance.builds && !instance.widget.keeps_children() {
        let env = instance.env;
        let keeps = !cx.everything && instance.built_in == cx.epoch;
        instance.built_in = cx.epoch;
        let born = build(cx, instance, env, keeps);
        reconcile_children(cx, instance, born, &env);
    }
    settle_aggregates(cx, instance);
}

fn build(cx: &mut ReconcileCx<'_>, instance: &mut Instance, env: Env, keeps: bool) -> Vec<Element> {
    let stamp = log::now();
    let marks = cx.marks;
    let (born, keys, replayed) = reads::dated(|| {
        serving(instance.id, marks, || {
            crate::ambient::keeping(keeps, || {
                instance
                    .widget
                    .build(instance.state.as_mut(), cx.theme, env)
            })
        })
    });
    let producer = Producer::Build(instance.id);
    cx.readers.remove(producer, &instance.build_reads);
    cx.readers.add(producer, &keys);
    instance.build_reads = keys;
    instance.build_stamp = replayed.map_or(stamp, |since| since.min(stamp));
    born
}

fn settle_aggregates(cx: &mut ReconcileCx<'_>, instance: &mut Instance) {
    instance.refresh_behavior();
    let behavior = instance.behavior;
    let children = cx.layouts.children(instance.node);
    let within = |test: fn(&Instance) -> bool| {
        children
            .iter()
            .any(|child| cx.table.holds(*child) && test(cx.table.get(*child)))
    };
    instance.paints_offscreen = instance.anchor.is_some()
        || behavior.has(Behavior::PAINTS_OFFSCREEN)
        || within(|child| child.paints_offscreen);
    instance.singular = singular(behavior, instance.anchor) || within(|child| child.singular);
}

fn relayout(cx: &mut ReconcileCx<'_>, instance: &mut Instance) {
    restyle(cx.layouts, cx.scrollers, cx.table, instance, cx.theme);
}

fn resettle(cx: &mut ReconcileCx<'_>, instance: &mut Instance) {
    if instance.widget.settled_height(instance.state.as_ref()) != instance.settled {
        relayout(cx, instance);
    }
}

pub(crate) fn restyle(
    layouts: &mut Layouts,
    scrollers: &mut FxHashSet<Node>,
    table: &mut Table,
    instance: &mut Instance,
    theme: &ThemeHandle,
) {
    let settled = instance.widget.settled_height(instance.state.as_ref());
    instance.settled = settled;
    let layout = resolved(
        &*instance.widget,
        settled,
        instance.anchor,
        instance.style.as_deref(),
        theme,
        &instance.env,
    );
    if layout == instance.layout {
        return;
    }
    let bound = instance.bound();
    layouts.set_style(instance.node, layout);
    instance.layout = layout;
    match layout.scrolls.is_some() {
        true => scrollers.insert(instance.node),
        false => scrollers.remove(&instance.node),
    };
    if instance.bound() != bound {
        table.islands.mark_around(instance.node, layouts);
    }
}

pub(crate) fn remeasure(layouts: &mut Layouts, theme: &ThemeHandle, instance: &Instance) {
    let measure = instance.widget.measure(theme, &instance.env);
    if measure.as_ref() == layouts.leaf(instance.node).map(|leaf| &leaf.spec) {
        return;
    }
    layouts.set_measure(instance.node, measure);
}

fn note_singular(cx: &mut ReconcileCx<'_>, behavior: Behavior, id: u64, component: bool) -> bool {
    if behavior.has(Behavior::AUTOFOCUS) && behavior.focusable() && !cx.autofocus.contains(&id) {
        cx.autofocus.push(id);
    }
    if behavior.has(Behavior::MODAL) {
        cx.modals.push(id);
    }
    let claims = behavior.has(Behavior::TAKES_FOCUS) && behavior.focusable();
    if claims && !component {
        *cx.claimed = Some(id);
    }
    claims
}

struct Resolution {
    locals: Vec<(TypeId, Store)>,
    component: Option<Box<ComponentSlot>>,
}

fn resolve(
    cx: &mut ReconcileCx<'_>,
    element: &mut Element,
    previous_producer: &[(TypeId, Store)],
    parent_env: &Env,
) -> Resolution {
    if element.source.is_none() && element.deferred.is_none() {
        return Resolution {
            locals: Vec::new(),
            component: None,
        };
    }
    let mut component = element
        .source
        .is_some()
        .then(|| view(cx, element, parent_env));
    let theme = cx.theme;
    let (locals, keys) = reads::recorded(|| {
        crate::ambient::with_env(theme, *parent_env, || {
            resolve_deferred(element, previous_producer)
        })
    });
    match &mut component {
        Some(component) => {
            component.reads.extend(keys);
            component.reads.sort_unstable();
            component.reads.dedup();
        }
        None => cx.attribute(cx.producer, keys),
    }
    Resolution { locals, component }
}

fn view(cx: &mut ReconcileCx<'_>, element: &mut Element, parent_env: &Env) -> Box<ComponentSlot> {
    let outer = Outer::from_element(element);
    let first = element
        .source
        .take()
        .expect("component element without source");
    let identity = first.identity();
    let built = log::now();
    let mut source = first;
    let mut outermost = None;
    let mut reads = Vec::new();
    loop {
        let chain: Vec<Rc<Scoped>> = cx.chain.iter().chain(&element.scopes).cloned().collect();
        let theme = cx.theme;
        let state = &mut *cx.state;
        let (mut output, keys) = reads::recorded(|| {
            crate::stats::component_viewed();
            crate::ambient::with_env(theme, *parent_env, || {
                source.view(&mut StateAccess::new(state, &chain))
            })
        });
        reads.extend(keys);
        adopt_outer(&mut output, element);
        *element = output;
        let inner = element.source.take();
        if outermost.is_none() {
            outermost = Some(source);
        }
        match inner {
            Some(inner) => source = inner,
            None => break,
        }
    }
    reads.sort_unstable();
    reads.dedup();
    Box::new(ComponentSlot {
        identity,
        source: outermost,
        built,
        reads,
        outer,
        env: *parent_env,
        dirty: false,
    })
}

fn adopt_outer(inner: &mut Element, outer: &mut Element) {
    inner.key = inner.key.or(outer.key);
    inner.branch = match (outer.branch, inner.branch) {
        (Some(outer), Some(inner)) => Some(crate::view::hash_of((outer, inner))),
        (outer, inner) => outer.or(inner),
    };
    inner.anchor = inner.anchor.or(outer.anchor);
    if inner.style.is_none() {
        inner.style = outer.style.take();
    }
    if inner.visual.is_none() {
        inner.visual = outer.visual.take();
    }
    if inner.tooltip.is_none() {
        inner.tooltip = outer.tooltip.take();
    }
    let mut scopes = std::mem::take(&mut outer.scopes);
    scopes.append(&mut inner.scopes);
    inner.scopes = scopes;
}

fn resolve_deferred(
    element: &mut Element,
    previous_producer: &[(TypeId, Store)],
) -> Vec<(TypeId, Store)> {
    let mut locals = Vec::new();
    while let Some(deferred) = element.deferred.take() {
        let identity = deferred.identity();
        let store = previous_producer
            .get(locals.len())
            .filter(|(stored_identity, _)| *stored_identity == identity)
            .map_or_else(|| deferred.store(), |(_, store)| Rc::clone(store));
        let mut resolved = deferred.resolve(&store);
        adopt_outer(&mut resolved, element);
        locals.push((identity, store));
        *element = resolved;
    }
    locals
}

fn hold(cx: &mut ReconcileCx<'_>, instance: &mut Instance, component: Option<Box<ComponentSlot>>) {
    let producer = Producer::View(instance.id);
    if let Some(old) = instance.component.take() {
        cx.readers.remove(producer, &old.reads);
    }
    if let Some(component) = &component {
        cx.readers.add(producer, &component.reads);
    }
    instance.component = component;
}

fn resolved(
    widget: &dyn AnyWidget,
    settled: Option<f32>,
    anchor: Option<Anchor>,
    style: Option<&Style>,
    theme: &ThemeHandle,
    env: &Env,
) -> Layout {
    let mut layout = widget.layout(theme, env);
    if let Some(style) = style {
        style.apply(&mut layout);
    }
    if let Some(height) = settled {
        layout.size.height = Length::px(height);
    }
    match anchor.is_some() {
        true => layout.anchored(),
        false => layout,
    }
}

pub(crate) fn create(cx: &mut ReconcileCx<'_>, mut element: Element, parent_env: &Env) -> Node {
    crate::stats::reconciled();
    let resolution = resolve(cx, &mut element, &[], parent_env);
    instantiate(cx, element, resolution, parent_env)
}

fn surroundings(cx: &ReconcileCx<'_>, element: &Element, parent_env: &Env) -> Env {
    let mut env = *parent_env;
    element.widget.surroundings(cx.theme, &mut env);
    if let Some(visual) = element.visual.as_ref() {
        if let Some(foreground) = visual.foreground {
            env.text_color = foreground;
        }
        env.carried |= visual.transition.is_some();
    }
    env
}

fn instantiate(
    cx: &mut ReconcileCx<'_>,
    element: Element,
    resolution: Resolution,
    parent_env: &Env,
) -> Node {
    let Resolution { locals, component } = resolution;
    let env = surroundings(cx, &element, parent_env);

    *cx.has_anchors |= element.anchor.is_some();
    let state = element.widget.init_state();
    let settled = element.widget.settled_height(state.as_ref());
    let layout = resolved(
        &*element.widget,
        settled,
        element.anchor,
        element.style.as_deref(),
        cx.theme,
        &env,
    );
    let measure = element.widget.measure(cx.theme, &env);
    let node = cx.layouts.create(layout, measure);
    let id = cx.table.id_for(node);
    if layout.scrolls.is_some() {
        cx.scrollers.insert(node);
    }
    if element.widget.arranges() {
        cx.arrangers.insert(node);
    }

    let memo = element.widget.memo().map(|digest| (digest, cx.epoch));
    let builds = element.children.is_empty();
    let arriving = cx.arrivals
        && element
            .visual
            .as_ref()
            .is_some_and(|visual| visual.transition.is_some());
    let passage = arriving.then(|| Box::new(Passage::arriving()));
    let behavior = element.widget.behavior(state.as_ref());
    let mut instance = Box::new(Instance {
        id,
        node,
        state,
        widget: element.widget,
        layout,
        settled,
        key: element.key,
        branch: element.branch,
        anchor: element.anchor,
        style: element.style,
        visual: element.visual,
        tooltip: element.tooltip,
        scopes: element.scopes,
        locals,
        component: None,
        motions: crate::motion::Motions::default(),
        passage,
        leaving: None,
        placed: None,
        frame_curve: None,
        visual_fade: None,
        builds,
        memo,
        built_in: cx.epoch,
        env,
        lane: 0.0,
        content_size: rikka_types::Size::ZERO,
        prims: Recycled::new(),
        fades: Vec::new(),
        sections: Vec::new(),
        subtree_fade: None,
        asked_build: false,
        wants_build: false,
        claimed: false,
        paints_offscreen: false,
        singular: false,
        behavior,
        build_reads: Vec::new(),
        build_stamp: 0,
        shifts: false,
        moves: false,
        hosts: false,
        imaged: false,
        last_context: None,
        drawn: None,
        outputs: Outputs::default(),
        told: Point::ZERO,
    });
    hold(cx, &mut instance, component);
    let born = match builds {
        true => build(cx, &mut instance, env, false),
        false => element.children,
    };
    let producer = producing(&instance, cx.producer);
    let scopes = instance.scopes.clone();
    let children: Vec<Node> = cx.descend(&scopes, producer, |cx| {
        born.into_iter()
            .map(|child| create(cx, child, &env))
            .collect()
    });
    cx.layouts.set_children(node, &children);
    let mut last = node;
    for &child in &children {
        cx.table.labels.join(last, child);
        last = last_of(cx.layouts, child);
    }
    instance.refresh_behavior();
    instance.claimed = note_singular(cx, instance.behavior, id, false);
    settle_aggregates(cx, &mut instance);
    cx.table.put(node, instance);
    node
}

pub(crate) fn reconcile(
    cx: &mut ReconcileCx<'_>,
    node: Node,
    mut element: Element,
    parent_env: &Env,
) -> Node {
    if element.is_kept() {
        crate::stats::retained();
        cx.retained = true;
        rebuild_walk(cx, node);
        return node;
    }
    if keeps(cx, node, &element, parent_env) {
        crate::stats::retained();
        cx.retained = true;
        crate::runtime::verify::verify(cx, node, element, parent_env);
        rebuild_walk(cx, node);
        return node;
    }
    crate::stats::reconciled();
    let mut instance = cx.table.take(node);
    let resolution = resolve(cx, &mut element, &instance.locals, parent_env);
    if element.widget.widget_type() != instance.widget.widget_type() {
        cx.table.put(node, instance);
        teardown(cx, node);
        return instantiate(cx, element, resolution, parent_env);
    }
    reconcile_taken(cx, &mut instance, element, resolution, parent_env);
    cx.table.put(node, instance);
    node
}

fn keeps(cx: &ReconcileCx<'_>, node: Node, element: &Element, parent_env: &Env) -> bool {
    if cx.everything || element.source.is_none() {
        return false;
    }
    let instance = cx.table.get(node);
    instance.leaving.is_none()
        && instance.built_in == cx.epoch
        && instance
            .component
            .as_ref()
            .is_some_and(|component| component.keeps(element, parent_env))
}

fn reconcile_taken(
    cx: &mut ReconcileCx<'_>,
    instance: &mut Instance,
    element: Element,
    resolution: Resolution,
    parent_env: &Env,
) {
    let Resolution { locals, component } = resolution;
    let env = surroundings(cx, &element, parent_env);
    let same = instance.widget.same(&*element.widget)
        && element.visual == instance.visual
        && element.style == instance.style
        && element.anchor == instance.anchor
        && env == instance.env
        && !instance.wants_build
        && instance.built_in == cx.epoch
        && instance.builds == element.children.is_empty();
    instance.locals = locals;
    hold(cx, instance, component);
    instance.widget = element.widget;
    instance.key = element.key;
    instance.branch = element.branch;
    instance.scopes = element.scopes;
    instance.tooltip = element.tooltip;
    *cx.has_anchors |= element.anchor.is_some();
    instance.refresh_behavior();
    instance.claimed = note_singular(cx, instance.behavior, instance.id, instance.claimed);
    if same {
        resettle(cx, instance);
        if !instance.builds {
            reconcile_children(cx, instance, element.children, &env);
        }
        settle_aggregates(cx, instance);
        return;
    }
    cx.table.mark(cx.layouts, instance.node, PAINT);
    instance.visual = element.visual;
    instance.style = element.style;
    let anchored = instance.anchor.is_some() != element.anchor.is_some();
    instance.anchor = element.anchor;
    let previous_producer = instance.env;
    instance.env = env;
    if anchored {
        cx.table.islands.mark_around(instance.node, cx.layouts);
    }

    relayout(cx, instance);
    remeasure(cx.layouts, cx.theme, instance);

    let memo = instance.widget.memo().map(|digest| (digest, cx.epoch));
    let children_retained = instance.widget.keeps_children()
        || (memo.is_some()
            && memo == instance.memo
            && previous_producer == env
            && !instance.wants_build);
    instance.memo = memo;
    if !children_retained {
        instance.asked_build = false;
        instance.wants_build = false;
        instance.builds = element.children.is_empty();
        instance.built_in = cx.epoch;
        let born = match instance.builds {
            true => build(cx, instance, env, false),
            false => element.children,
        };
        reconcile_children(cx, instance, born, &env);
    }
    settle_aggregates(cx, instance);
}

fn reconcile_children(
    cx: &mut ReconcileCx<'_>,
    parent: &mut Instance,
    elements: Vec<Element>,
    env: &Env,
) {
    if elements.is_empty() && cx.layouts.children(parent.node).is_empty() {
        return;
    }
    let producer = producing(parent, cx.producer);
    let scopes = parent.scopes.clone();
    let theme = cx.theme;
    crate::ambient::with_env(theme, *env, || {
        cx.descend(&scopes, producer, |cx| {
            match_children(cx, parent.node, elements, env);
        });
    });
}

fn match_children(cx: &mut ReconcileCx<'_>, parent: Node, elements: Vec<Element>, env: &Env) {
    let previous_producer: Scratch<Node> = cx.layouts.children(parent).iter().copied().collect();
    if settled_match(cx.table, &previous_producer, &elements) {
        let mut next: Scratch<Node> = Scratch::new();
        for (node, element) in std::iter::zip(previous_producer.iter(), elements) {
            next.push(reconcile(cx, *node, element, env));
        }
        settle_children(cx, parent, &previous_producer, &next);
        return;
    }
    let leaving = |child: &Node| cx.table.get(*child).leaving.is_some();
    let mut ghosts: Scratch<Node> = previous_producer.iter().copied().filter(leaving).collect();
    let mut old: Scratch<Option<Node>> = previous_producer
        .iter()
        .copied()
        .filter(|child| !leaving(child))
        .map(Some)
        .collect();
    let mut keyed: Scratch<(u64, usize)> = old
        .iter()
        .enumerate()
        .filter_map(|(index, slot)| {
            let key = cx.table.get((*slot)?).key?;
            Some((key, index))
        })
        .collect();
    keyed.sort_unstable();
    let mut next: Scratch<Node> = Scratch::new();
    let mut created: Scratch<Node> = Scratch::new();
    let positional = PositionalMatch::compute(cx.table, cx.layouts, &old, &elements);

    for (index, element) in elements.into_iter().enumerate() {
        let position = match element.key {
            Some(_) => Some(index),
            None => positional.position(index),
        };
        let matched = position
            .and_then(|position| take_match(cx.table, &mut old, &mut keyed, position, &element));
        match matched {
            Some(matched) => next.push(reconcile(cx, matched, element, env)),
            None if element.is_kept() => {
                debug_assert!(false, "a kept child names no previous_producer child");
                tracing::error!(key = ?element.key, "a kept child names no previous_producer child");
            }
            None => match revivable(cx.table, &ghosts, index, &element) {
                Some(at) => {
                    let ghost = ghosts.remove(at);
                    revive(cx, ghost);
                    next.push(reconcile(cx, ghost, element, env));
                }
                None => {
                    let born = create(cx, element, env);
                    created.push(born);
                    next.push(born);
                }
            },
        }
    }

    let mut doomed: Scratch<Node> = Scratch::new();
    for (index, leftover) in old.iter().copied().enumerate() {
        let Some(leftover) = leftover else {
            continue;
        };
        let instance = cx.table.get_mut(leftover);
        let departs = instance
            .visual
            .as_ref()
            .is_some_and(|visual| visual.transition.is_some());
        if !departs {
            unlabel(cx, leftover);
            doomed.push(leftover);
            continue;
        }
        if let Some(placed) = instance.placed {
            cx.layouts.fix(leftover, Fix::Retained(placed));
        }
        let drawn = (!cx.table.has(leftover, CULLED)
            && cx.table.get(leftover).last_context.is_some())
        .then(|| cx.table.seen(cx.layouts, leftover).origin);
        let instance = cx.table.get_mut(leftover);
        instance.leaving = Some(Exit::new(index, drawn));
        cx.table.refresh_behavior(leftover);
        cx.table.mark(cx.layouts, leftover, PAINT);
        ghosts.push(leftover);
    }

    ghosts.sort_by_key(|ghost| {
        cx.table
            .get(*ghost)
            .leaving
            .map_or(0, |leaving| leaving.index)
    });
    for &ghost in ghosts.iter() {
        let at = cx
            .table
            .get(ghost)
            .leaving
            .map_or(next.len(), |leaving| leaving.index.min(next.len()));
        next.insert(at, ghost);
    }

    replace_children(cx, parent, &next);
    for &leftover in doomed.iter() {
        discard(cx, leftover);
    }
    for &born in created.iter() {
        cx.table.mark(cx.layouts, born, PAINT);
    }
}

fn revivable(table: &Table, ghosts: &[Node], index: usize, element: &Element) -> Option<usize> {
    let identity = element.identity();
    ghosts.iter().position(|ghost| {
        let instance = table.get(*ghost);
        instance.leaving.is_some_and(|leaving| {
            !leaving.done && (element.key.is_some() || leaving.index == index)
        }) && instance.key == element.key
            && instance.branch == element.branch
            && instance.identity() == identity
    })
}

fn revive(cx: &mut ReconcileCx<'_>, ghost: Node) {
    cx.layouts.release(ghost);
    cx.table.get_mut(ghost).leaving = None;
    cx.table.refresh_behavior(ghost);
    cx.table.islands.mark_around(ghost, cx.layouts);
    cx.table.mark(cx.layouts, ghost, PAINT);
    cx.retained = true;
}

fn order_children(cx: &mut ReconcileCx<'_>, parent: Node, children: &[Node]) {
    let mut anchor = parent;
    for &child in children {
        let last = last_of(cx.layouts, child);
        if !cx.table.labels.follows(child, anchor) {
            cx.table.labels.detach(child, last);
            cx.table.labels.attach_after(Some(anchor), child, last);
        }
        anchor = last;
    }
}

fn settled_match(table: &Table, children: &[Node], elements: &[Element]) -> bool {
    children.len() == elements.len()
        && std::iter::zip(children, elements)
            .enumerate()
            .all(|(at, (node, element))| {
                if let Some(&ahead) = children.get(at + MATCH_AHEAD) {
                    table.prefetch_head(ahead, MATCHED_HEAD);
                }
                let instance = table.get(*node);
                instance.leaving.is_none()
                    && instance.key == element.key
                    && (element.is_kept()
                        || (instance.branch == element.branch
                            && instance.identity() == element.identity()))
            })
}

struct PositionalMatch {
    old_len: usize,
    new_len: usize,
    common_suffix: usize,
}

impl PositionalMatch {
    fn compute(
        table: &Table,
        layouts: &Layouts,
        old: &[Option<Node>],
        elements: &[Element],
    ) -> PositionalMatch {
        let (old_len, new_len) = (old.len(), elements.len());
        let common_suffix = match old_len == new_len {
            true => 0,
            false => std::iter::zip(old.iter().rev(), elements.iter().rev())
                .take_while(|(node, element)| {
                    element.key.is_none()
                        && node.is_some_and(|node| is_same_widget(table, layouts, node, element))
                })
                .count(),
        };
        PositionalMatch {
            old_len,
            new_len,
            common_suffix,
        }
    }

    fn position(&self, index: usize) -> Option<usize> {
        let suffix_start = self.new_len - self.common_suffix;
        match index >= suffix_start {
            true => Some(index - suffix_start + self.old_len - self.common_suffix),
            false => (index < self.old_len - self.common_suffix).then_some(index),
        }
    }
}

fn is_same_widget(table: &Table, layouts: &Layouts, node: Node, element: &Element) -> bool {
    let instance = table.get(node);
    let own = instance.key == element.key
        && instance.identity() == element.identity()
        && instance.branch == element.branch
        && instance.style == element.style
        && instance.visual == element.visual
        && instance.builds == element.children.is_empty()
        && instance.widget.same(&*element.widget);
    if !own {
        return false;
    }
    if instance.builds {
        return true;
    }
    let mut previous_producer = layouts
        .children(node)
        .iter()
        .copied()
        .filter(|child| table.get(*child).leaving.is_none());
    let mut coming = element.children.iter();
    loop {
        match (previous_producer.next(), coming.next()) {
            (None, None) => return true,
            (Some(child), Some(element)) if is_same_widget(table, layouts, child, element) => {}
            _ => return false,
        }
    }
}

fn take_match(
    table: &Table,
    old: &mut [Option<Node>],
    keyed: &mut [(u64, usize)],
    index: usize,
    element: &Element,
) -> Option<Node> {
    let identity = element.identity();
    let fits = |node: &Node| {
        let instance = table.get(*node);
        element.is_kept() || (instance.identity() == identity && instance.branch == element.branch)
    };
    if let Some(key) = element.key {
        let found = keyed.binary_search_by_key(&key, |(key, _)| *key).ok()?;
        let position = std::mem::replace(&mut keyed[found].1, TAKEN);
        let slot = old.get_mut(position)?;
        if slot.as_ref().is_some_and(fits) {
            slot.take()
        } else {
            None
        }
    } else {
        let slot = old.get_mut(index)?;
        if slot
            .as_ref()
            .is_some_and(|node| table.get(*node).key.is_none() && fits(node))
        {
            slot.take()
        } else {
            None
        }
    }
}

const TAKEN: usize = usize::MAX;

pub(crate) fn teardown(cx: &mut ReconcileCx<'_>, node: Node) {
    unlabel(cx, node);
    if let Some(parent) = cx.layouts.parent(node) {
        cx.table.islands.mark(parent);
    }
    discard(cx, node);
}

fn unlabel(cx: &mut ReconcileCx<'_>, node: Node) {
    let last = last_of(cx.layouts, node);
    cx.table.labels.detach(node, last);
}

fn discard(cx: &mut ReconcileCx<'_>, node: Node) {
    release(cx, node);
    cx.layouts.remove(node);
}

fn release(cx: &mut ReconcileCx<'_>, node: Node) {
    for index in 0..cx.layouts.children(node).len() {
        let child = cx.layouts.children(node)[index];
        if cx.layouts.parent(child) == Some(node) {
            release(cx, child);
        }
    }
    let Some(mut instance) = cx.table.remove(node) else {
        return;
    };
    cx.readers
        .remove(Producer::Build(instance.id), &instance.build_reads);
    if let Some(component) = instance.component.take() {
        cx.readers
            .remove(Producer::View(instance.id), &component.reads);
    }
    forget_prims(cx.scene, cx.images, &mut instance.prims);
    for layer in instance.fades.drain(..) {
        cx.scene.remove_layer(layer);
    }
    for scrolled in instance.sections.drain(..) {
        cx.scene.remove_group(scrolled.group);
    }
    if let Some(layer) = instance.subtree_fade.take() {
        cx.scene.remove_layer(layer);
    }
    if let Some(layer) = instance.visual_fade.take() {
        cx.scene.remove_layer(layer);
    }
    cx.scrollers.remove(&node);
    cx.arrangers.remove(&node);
    cx.hosting.remove(&node);
    cx.imaged.remove(&node);
    cx.graveyard.hold(instance);
}
