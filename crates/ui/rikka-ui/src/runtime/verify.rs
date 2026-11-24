use crate::runtime::layout::{Layouts, Node};
use crate::runtime::reconcile::{ReconcileCx, unfold};
use crate::runtime::table::Table;
use crate::view::observe::log;
use crate::view::{Element, Env};

pub(crate) fn verify(cx: &mut ReconcileCx<'_>, node: Node, mut element: Element, env: &Env) {
    if !cfg!(any(debug_assertions, test)) || log::now() != cx.taken_at {
        return;
    }
    let Some(source) = element.source.take() else {
        return;
    };
    {
        let _quiet = log::Quiet::enter();
        let _muted = crate::stats::MutedGuard::enter();
        unfold(cx, &mut element, &*source, env);
    }
    if let Some(difference) = differs(cx.table, cx.layouts, node, &element, true) {
        let name = cx
            .table
            .get(node)
            .component
            .as_ref()
            .map_or("component", |component| component.name());
        panic!(
            "component `{name}` rebuilt to {difference} without a tracked read changing; \
             its view reads an `#[untracked]` field, an untracked field of tracked state, \
             or shared state that is not `Observed`"
        );
    }
}

fn differs(
    table: &Table,
    layouts: &Layouts,
    node: Node,
    element: &Element,
    top: bool,
) -> Option<String> {
    let instance = table.get(node);
    if let Some(source) = &element.source {
        let same = instance
            .component
            .as_ref()
            .and_then(|component| component.source.as_ref())
            .is_some_and(|component| component.same(&**source));
        return (!same).then(|| format!("another `{}`", source.name()));
    }
    if element.deferred.is_some() || (top && !instance.locals.is_empty()) {
        return None;
    }
    if instance.widget.widget_type() != element.widget.widget_type() {
        return Some(format!(
            "a `{}` where it showed a `{}`",
            element.widget.debug_name(),
            instance.widget.debug_name()
        ));
    }
    let named = instance.widget.debug_name();
    if !top && (instance.key != element.key || instance.branch != element.branch) {
        return Some(format!("a `{named}` under another key"));
    }
    if !top && (instance.style != element.style || instance.visual != element.visual) {
        return Some(format!("a `{named}` styled otherwise"));
    }
    if !top && instance.tooltip != element.tooltip {
        return Some(format!("a `{named}` with another tooltip"));
    }
    if instance.widget.same(&*instance.widget) && !instance.widget.same(&*element.widget) {
        return Some(format!("a different `{named}`"));
    }
    if instance.builds {
        return None;
    }
    let children: Vec<Node> = layouts
        .children(node)
        .iter()
        .copied()
        .filter(|child| table.get(*child).leaving.is_none())
        .collect();
    if children.len() != element.children.len() {
        return Some(format!(
            "{} children in a `{named}` that had {}",
            element.children.len(),
            children.len()
        ));
    }
    std::iter::zip(children, &element.children)
        .find_map(|(child, element)| differs(table, layouts, child, element, false))
}
