use std::sync::Arc;

use animate_look::GraphRef;
use animate_shader::Status;
use animate_ui::glyph;
use rikka_app::prelude::*;
use rikka_shader_graph::{NodeKind, Scope};
use rikka_widgets::node_canvas::{
    CanvasEdit, CanvasMenu, Hit, Item, NodeGraph, Selection, node_canvas,
};

use crate::actions::{Starter, users};
use crate::canvas_data::node_id;
use crate::state::{Popup, Renaming, Tab};
use crate::traits::{Desk, Workshop};
use crate::views::{catalog, slots, wording};

const CANVAS: &str = "animate-shader-canvas";
const CONTEXT: &str = "animate-shader-context";
const MATERIALS: &str = "animate-shader-materials";
const SCOPES: &str = "animate-shader-scopes";
const PROBLEMS: &str = "animate-shader-problems";
const MENU_WIDTH: f32 = 220.0;
const COMBO: f32 = 170.0;
const NAMING: f32 = 160.0;
const PROBLEMS_WIDTH: f32 = 360.0;

pub fn node_editor(desk: Desk<'_>) -> impl View {
    let shader = desk.shader();
    let graph = match shader.graph() {
        Some(_) => canvas_area(desk).any(),
        None => status_page(MaterialSymbol::SHAPES, tr!("animate.shader.no_materials"))
            .action(
                button(text(tr!("animate.shader.new_material")))
                    .small()
                    .focusless()
                    .on_press(desk.act(|studio| {
                        studio.add_material(Starter::Empty);
                    })),
            )
            .fill()
            .any(),
    };
    let body = match (shader.tab(), desk.slot_figure()) {
        (Tab::Material, Some(figure)) => row_of([
            slots::slots(desk, figure),
            rule_upright().any(),
            container(graph).grow(1.0).fill_height().any(),
        ])
        .fill()
        .any(),
        _ => graph,
    };
    let mut parts = vec![
        header(desk).any(),
        rule().any(),
        container(body).grow(1.0).fill_width().any(),
    ];
    if let Some(notice) = shader.notice() {
        parts.push(
            container(
                row_of([
                    text(notice.to_owned())
                        .caption()
                        .color(palette().danger)
                        .grow(1.0)
                        .any(),
                    glyph(MaterialSymbol::CLOSE)
                        .ghost()
                        .on_press(desk.act(|studio| studio.dismiss_notice()))
                        .tooltip(tr!("animate.shader.dismiss")),
                ])
                .gap(Space::TIGHT)
                .align_items(Align::Center)
                .fill_width(),
            )
            .padding(Inset::xy(12.0, 4.0))
            .fill_width()
            .any(),
        );
    }
    container(column_of(parts).fill())
        .background(palette().surface)
        .fill()
}

fn header(desk: Desk<'_>) -> impl View {
    let shader = desk.shader();
    let mut popups = Vec::new();
    let mut bar = toolbar()
        .padding(Inset::xy(12.0, 6.0))
        .group(tab_tools(desk).priority(ToolPriority::High));
    if shader.tab() == Tab::Material {
        if shader.popup == Some(Popup::Materials) {
            popups.push(materials_menu(desk));
        }
        bar = bar.group(material_tools(desk).priority(ToolPriority::High));
    }
    if let Some(at) = shader.graph() {
        if shader.popup == Some(Popup::Scopes) {
            popups.push(scopes_menu(desk));
        }
        popups.extend(problems_list(desk, &at));
        bar = bar
            .group(scope_tools(desk))
            .spacer()
            .group(history_tools(desk).priority(ToolPriority::Low))
            .group(
                tool_group([tool_item(problems_chip(desk, &at))]).priority(ToolPriority::Pinned),
            );
    }
    column_of(std::iter::once(bar.any()).chain(popups)).fill_width()
}

fn tab_tools(desk: Desk<'_>) -> ToolGroup {
    let tab = desk.shader().tab();
    let chosen = Tab::ALL.iter().position(|each| *each == tab);
    let show = desk.act_with(|studio, index: usize| {
        if let Some(tab) = Tab::ALL.get(index) {
            studio.show_tab(*tab);
        }
    });
    let tabs = segmented(chosen.unwrap_or(0))
        .segments(Tab::ALL.map(wording::tab))
        .control_size(ControlSize::Small)
        .focusless()
        .on_select(show.clone());
    tool_group([
        tool_item(tabs).overflow([menu_item(tr!("animate.shader.tab.graph"))
            .icon(MaterialSymbol::SHAPES)
            .choices(Tab::ALL.map(wording::tab), chosen, show)]),
    ])
}

fn naming_field(desk: Desk<'_>) -> Element {
    text_field(&desk.shader().naming)
        .control_size(ControlSize::Small)
        .autofocus(true)
        .on_submit(desk.act(|studio| studio.finish_renaming()))
        .on_blur(desk.act(|studio| studio.finish_renaming()))
        .width(NAMING)
        .any()
}

fn material_tools(desk: Desk<'_>) -> ToolGroup {
    let shader = desk.shader();
    let keys: Vec<String> = desk.look().materials.keys().cloned().collect();
    let chosen = shader.material.clone();
    let mut tools = Vec::new();
    match (&shader.renaming, &chosen) {
        (Some(Renaming::Material(_)), _) => tools.push(tool_item(naming_field(desk))),
        (_, Some(_)) => {
            let at = keys.iter().position(|key| Some(key) == chosen.as_ref());
            let shown = keys.clone();
            let show = desk.act_with(move |studio, index: usize| {
                if let Some(key) = shown.get(index) {
                    studio.show_material(key.clone());
                }
            });
            let picker = combo(at)
                .options(keys.clone())
                .on_select(show.clone())
                .focusless()
                .control_size(ControlSize::Small)
                .width(COMBO);
            tools.push(
                tool_item(picker).overflow([menu_item(tr!("animate.shader.tab.material"))
                    .icon(MaterialSymbol::PALETTE)
                    .choices(keys, at, show)]),
            );
        }
        (_, None) => {}
    }
    if let Some(key) = &chosen {
        tools.push(tool_item(
            text(tr!(
                "animate.shader.used_by",
                count = users(desk.look(), key)
            ))
            .caption()
            .muted()
            .no_wrap(),
        ));
    }
    let more = glyph(MaterialSymbol::MORE_VERT)
        .ghost()
        .on_press(desk.act(|studio| studio.toggle_popup(Popup::Materials)))
        .tooltip(tr!("animate.shader.materials"))
        .build()
        .keyed(MATERIALS);
    tools.push(
        tool_item(more).overflow([menu_item(tr!("animate.shader.materials"))
            .icon(MaterialSymbol::MORE_VERT)
            .items(material_items(desk, chosen))]),
    );
    tool_group(tools)
}

fn materials_menu(desk: Desk<'_>) -> Element {
    menu()
        .items(material_items(desk, desk.shader().material.clone()))
        .width(MENU_WIDTH)
        .on_dismiss(desk.act(|studio| studio.close_popup()))
        .build()
        .keyed((MATERIALS, "menu"))
        .anchored(Anchor::below(MATERIALS))
}

fn material_items(desk: Desk<'_>, chosen: Option<String>) -> Vec<MenuItem> {
    let mut starters = vec![
        menu_item(tr!("animate.shader.empty_material")).on_press(desk.act(|studio| {
            studio.add_material(Starter::Empty);
        })),
    ];
    for preset in rikka_shader_presets::presets() {
        starters.push(
            menu_item(Label::from(preset.title)).on_press(desk.act(move |studio| {
                studio.add_material(Starter::Preset(preset));
            })),
        );
    }
    let has = chosen.is_some();
    let renamed = chosen.unwrap_or_default();
    vec![
        menu_item(tr!("animate.shader.new_material"))
            .submenu(true)
            .items(starters),
        menu_item(tr!("animate.shader.duplicate_material"))
            .enabled(has)
            .on_press(desk.act(|studio| studio.duplicate_material())),
        menu_item(tr!("animate.shader.rename"))
            .enabled(has)
            .on_press(
                desk.act(move |studio| studio.start_renaming(Renaming::Material(renamed.clone()))),
            ),
        menu_item(tr!("animate.shader.remove_material"))
            .enabled(has)
            .on_press(desk.act(|studio| studio.remove_material())),
    ]
}

fn scope_tools(desk: Desk<'_>) -> ToolGroup {
    let shader = desk.shader();
    let path = shader.path.clone();
    let mut tools = vec![
        tool_item(rule_upright()),
        tool_item(crumb(
            desk,
            tr!("animate.shader.scope.main"),
            0,
            path.is_empty(),
        )),
    ];
    for (depth, step) in path.iter().enumerate() {
        tools.push(tool_item(
            icon(MaterialSymbol::CHEVRON_RIGHT).extent(Length::px(14.0)),
        ));
        let last = depth + 1 == path.len();
        tools.push(tool_item(match (&shader.renaming, last) {
            (Some(Renaming::Function(_)), true) => naming_field(desk),
            _ => crumb(desk, Label::from(step.clone()), depth + 1, last),
        }));
    }
    let groups = glyph(MaterialSymbol::ACCOUNT_TREE)
        .ghost()
        .on_press(desk.act(|studio| studio.toggle_popup(Popup::Scopes)))
        .tooltip(tr!("animate.shader.groups"))
        .build()
        .keyed(SCOPES);
    tools.push(
        tool_item(groups).overflow([menu_item(tr!("animate.shader.groups"))
            .icon(MaterialSymbol::ACCOUNT_TREE)
            .items(scope_items(desk))]),
    );
    tool_group(tools)
}

fn crumb(desk: Desk<'_>, label: Label, depth: usize, current: bool) -> Element {
    let base = button(text(label).caption().no_wrap()).small().focusless();
    match current {
        true => base.secondary().any(),
        false => base
            .ghost()
            .on_press(desk.act(move |studio| studio.leave_to(depth)))
            .any(),
    }
}

fn scopes_menu(desk: Desk<'_>) -> Element {
    menu()
        .items(scope_items(desk))
        .width(MENU_WIDTH)
        .on_dismiss(desk.act(|studio| studio.close_popup()))
        .build()
        .keyed((SCOPES, "menu"))
        .anchored(Anchor::below(SCOPES))
}

fn scope_items(desk: Desk<'_>) -> Vec<MenuItem> {
    let current = desk.shader().scope();
    let names: Vec<String> = desk
        .shown()
        .map(|(_, graph)| graph.functions().keys().cloned().collect())
        .unwrap_or_default();
    let mut items = vec![
        menu_item(tr!("animate.shader.scope.main"))
            .selected(current == Scope::Main)
            .on_press(desk.act(|studio| {
                studio.close_popup();
                studio.leave_to(0);
            })),
    ];
    for name in names {
        let scope = Scope::Function(name.clone());
        items.push(
            menu_item(Label::from(name))
                .selected(current == scope)
                .on_press(desk.act(move |studio| {
                    studio.close_popup();
                    studio.show_scope(&scope);
                })),
        );
    }
    items.push(menu_separator());
    items.push(
        menu_item(tr!("animate.shader.add_group"))
            .on_press(desk.act(|studio| studio.add_function())),
    );
    if let Scope::Function(name) = current {
        let removed = name.clone();
        items.push(menu_item(tr!("animate.shader.rename_group")).on_press(
            desk.act(move |studio| studio.start_renaming(Renaming::Function(name.clone()))),
        ));
        items.push(
            menu_item(tr!("animate.shader.remove_group"))
                .on_press(desk.act(move |studio| studio.remove_function(&removed))),
        );
    }
    items
}

fn history_tools(desk: Desk<'_>) -> ToolGroup {
    tool_group([
        tool_button(MaterialSymbol::UNDO, tr!("animate.shader.undo"))
            .enabled(desk.can_undo())
            .on_press(desk.act(|studio| studio.travel(true))),
        tool_button(MaterialSymbol::REDO, tr!("animate.shader.redo"))
            .enabled(desk.can_redo())
            .on_press(desk.act(|studio| studio.travel(false))),
        tool_button(MaterialSymbol::ACCOUNT_TREE, tr!("animate.shader.arrange"))
            .on_press(desk.act(|studio| studio.arrange())),
    ])
}

fn problems_chip(desk: Desk<'_>, at: &GraphRef) -> Element {
    let status = desk.status_of(at);
    let (label, color) = wording::status(&status);
    let failed = matches!(status, Status::Failed(_));
    button(text(label).caption().color(color).no_wrap())
        .ghost()
        .small()
        .focusless()
        .enabled(failed)
        .on_press(desk.act(|studio| studio.toggle_popup(Popup::Problems)))
        .build()
        .keyed(PROBLEMS)
}

fn problems_list(desk: Desk<'_>, at: &GraphRef) -> Option<Element> {
    let Status::Failed(diagnostics) = desk.status_of(at) else {
        return None;
    };
    if desk.shader().popup != Some(Popup::Problems) {
        return None;
    }
    let lines = diagnostics.iter().cloned().map(|diagnostic| {
        let located = diagnostic.node().is_some();
        button(
            text(diagnostic.to_string())
                .caption()
                .color(palette().danger)
                .fill_width(),
        )
        .ghost()
        .small()
        .focusless()
        .enabled(located)
        .on_press(desk.act(move |studio| studio.focus(&diagnostic)))
        .fill_width()
        .any()
    });
    Some(
        popover(
            column_of(lines)
                .gap(Space::TIGHT)
                .padding(Inset::all(8.0))
                .width(PROBLEMS_WIDTH),
        )
        .on_dismiss(desk.act(|studio| studio.close_popup()))
        .build()
        .keyed((PROBLEMS, "list"))
        .anchored(Anchor::below(PROBLEMS)),
    )
}

fn canvas_area(desk: Desk<'_>) -> impl View {
    let shader = desk.shader();
    let mut layers = vec![spacer().height(0.0).fill_width().build().keyed(CANVAS)];
    if let Some(at) = shader.graph() {
        let diagnostics = desk.diagnostics_of(&at);
        if let Some(graph) = shader.canvas_graph(desk.look(), desk.workbench(), &diagnostics) {
            layers.push(
                container(canvas(desk, graph))
                    .grow(1.0)
                    .fill_width()
                    .build()
                    .keyed((
                        "animate-shader-graph",
                        format!("{at:?}"),
                        format!("{:?}", shader.scope()),
                    )),
            );
        }
    }
    if let Some(menu) = shader.menu {
        layers.push(match menu.hit {
            Hit::Node(_) | Hit::Link(_) | Hit::FrameTitle(_) | Hit::Frame(_)
                if menu.from.is_none() =>
            {
                context_menu(desk, menu)
            }
            _ => catalog::catalog(desk, menu),
        });
    }
    column_of(layers).fill()
}

fn canvas(desk: Desk<'_>, graph: Arc<NodeGraph>) -> impl View {
    node_canvas(graph, desk.shader().selection().clone())
        .on_select(desk.act_with(|studio, selection: Selection| studio.select(selection)))
        .on_edit(desk.act_with(|studio, edit: CanvasEdit| studio.canvas_edit(edit)))
        .on_menu(desk.act_with(|studio, menu: CanvasMenu| studio.open_menu(menu)))
        .on_activate(desk.act_with(|studio, item: Item| activate(studio, item)))
        .fill()
}

fn activate(studio: &mut dyn Workshop, item: Item) {
    if let Item::Node(key) = item
        && let Some((_, graph)) = studio.shown()
        && let Some(body) = graph.scope(&studio.shader().scope())
        && let Some(id) = node_id(body, key)
        && let Some(NodeKind::Call(name)) = body.node(id).map(|node| node.kind.clone())
    {
        studio.enter(&name);
        return;
    }
    studio.select(Selection::of([item]));
}

pub(crate) fn at_pointer(menu: &CanvasMenu) -> Anchor {
    Anchor::below(CANVAS).offset(menu.local.x, menu.local.y)
}

fn context_menu(desk: Desk<'_>, request: CanvasMenu) -> Element {
    let selection = desk.shader().selection().clone();
    let target = match request.hit {
        Hit::Node(key) if !selection.contains(Item::Node(key)) => Selection::of([Item::Node(key)]),
        Hit::Link(link) => Selection::of([Item::Link(link)]),
        Hit::FrameTitle(key) | Hit::Frame(key) => Selection::of([Item::Frame(key)]),
        _ => selection,
    };
    let call = match request.hit {
        Hit::Node(key) => desk.shown().and_then(|(_, graph)| {
            let body = graph.scope(&desk.shader().scope())?;
            let id = node_id(body, key)?;
            match &body.node(id)?.kind {
                NodeKind::Call(name) => Some((id, name.clone())),
                _ => None,
            }
        }),
        _ => None,
    };
    let nodes = target.nodes().count();
    let edit = |label: Label, edit: CanvasEdit| {
        menu_item(label).on_press(desk.act(move |studio| {
            studio.close_menu();
            studio.canvas_edit(edit.clone());
        }))
    };
    let on_target = |label: Label, act: fn(&mut dyn Workshop)| {
        let target = target.clone();
        menu_item(label).on_press(desk.act(move |studio| {
            studio.close_menu();
            studio.select(target.clone());
            act(studio);
        }))
    };
    let at = request.at;
    let mut items = vec![
        edit(
            tr!("animate.shader.duplicate"),
            CanvasEdit::Duplicate(target.clone()),
        ),
        edit(tr!("animate.shader.copy"), CanvasEdit::Copy(target.clone())),
        edit(tr!("animate.shader.cut"), CanvasEdit::Cut(target.clone())),
        menu_item(tr!("animate.shader.paste"))
            .enabled(desk.shader().clipboard.is_some())
            .on_press(desk.act(move |studio| {
                studio.close_menu();
                studio.paste(at);
            })),
        menu_separator(),
        on_target(tr!("animate.shader.frame_nodes"), |studio| {
            studio.frame_selection()
        })
        .enabled(nodes > 0),
        menu_item(tr!("animate.shader.add_comment")).on_press(desk.act(move |studio| {
            studio.close_menu();
            studio.add_comment(at);
        })),
        on_target(tr!("animate.shader.make_group"), |studio| {
            studio.collapse_selection()
        })
        .enabled(nodes > 0),
    ];
    if let Some((id, name)) = call {
        items.push(
            menu_item(tr!("animate.shader.enter_group")).on_press(desk.act(move |studio| {
                studio.close_menu();
                studio.enter(&name);
            })),
        );
        items.push(
            menu_item(tr!("animate.shader.ungroup")).on_press(desk.act(move |studio| {
                studio.close_menu();
                studio.expand_node(id);
            })),
        );
    }
    items.push(menu_separator());
    items.push(edit(
        tr!("animate.shader.delete"),
        CanvasEdit::Delete(target),
    ));
    menu()
        .items(items)
        .width(MENU_WIDTH)
        .on_dismiss(desk.act(|studio| studio.close_menu()))
        .build()
        .keyed(CONTEXT)
        .anchored(at_pointer(&request))
}
