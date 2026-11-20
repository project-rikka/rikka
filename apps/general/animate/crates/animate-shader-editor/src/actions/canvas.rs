use std::collections::BTreeSet;

use rikka_app::prelude::*;
use rikka_shader_graph::{
    Change, Comment, Connection, Edit, Fragment, Graph, Group, Library, NodeId, NodeKind, PortRef,
    Scope, ValueType,
};
use rikka_widgets::node_canvas::{CanvasEdit, FrameKey, Item, PortAddr, PortSide, Selection};

use crate::canvas_data::{FrameId, frame_id, node_id, node_key, ports_of};
use crate::catalog::{accepts, produces};
use crate::traits::Workshop;

const PASTE_SHIFT: f32 = 40.0;
const COMMENT_SIZE: [f32; 2] = [240.0, 120.0];

struct Resolved {
    node: NodeId,
    port: String,
    ty: Option<ValueType>,
    many: bool,
}

fn resolve(graph: &Graph, library: &Library, scope: &Scope, addr: PortAddr) -> Option<Resolved> {
    let body = graph.scope(scope)?;
    let node = node_id(body, addr.node)?;
    let report = graph.validate(library);
    let ports = ports_of(graph, library, &report, scope, node);
    let (port, ty) = match addr.side {
        PortSide::Input => {
            let port = ports.inputs.get(addr.index)?;
            (port, report.input_type(scope, node, &port.name))
        }
        PortSide::Output => {
            let port = ports.outputs.get(addr.index)?;
            (port, report.output_type(scope, node, &port.name))
        }
    };
    Some(Resolved {
        node,
        port: port.name.clone(),
        ty,
        many: port.many,
    })
}

fn selected_nodes(graph: &Graph, scope: &Scope, selection: &Selection) -> BTreeSet<NodeId> {
    graph
        .scope(scope)
        .map(|body| {
            selection
                .nodes()
                .filter_map(|key| node_id(body, key))
                .collect()
        })
        .unwrap_or_default()
}

fn batch(
    graph: &mut Graph,
    library: &Library,
    edits: Vec<Edit>,
) -> Result<Change<()>, rikka_shader_graph::GraphError> {
    Ok(Change {
        undo: graph.apply(library, Edit::Batch(edits))?,
        value: (),
    })
}

impl dyn Workshop + '_ {
    pub(crate) fn selected_nodes(&self) -> BTreeSet<NodeId> {
        let Some((_, graph)) = self.shown() else {
            return BTreeSet::new();
        };
        selected_nodes(graph, &self.shader().scope(), self.shader().selection())
    }

    fn frame(&self, key: FrameKey) -> Option<FrameId> {
        let (_, graph) = self.shown()?;
        frame_id(graph.scope(&self.shader().scope())?, key)
    }

    pub(crate) fn canvas_edit(&mut self, edit: CanvasEdit) {
        match edit {
            CanvasEdit::Move(moves) => self.move_nodes(&moves),
            CanvasEdit::MoveFrame(key, at) => self.reshape_comment(key, Some([at.x, at.y]), None),
            CanvasEdit::ResizeFrame(key, size) => {
                self.reshape_comment(key, None, Some([size.width, size.height]))
            }
            CanvasEdit::Connect {
                from,
                to,
                replacing,
            } => self.connect(from, to, replacing),
            CanvasEdit::Disconnect(input) => self.disconnect(input),
            CanvasEdit::Delete(selection) => self.delete(&selection),
            CanvasEdit::Copy(selection) => self.copy(&selection),
            CanvasEdit::Cut(selection) => {
                self.copy(&selection);
                self.delete(&selection);
            }
            CanvasEdit::Duplicate(selection) => {
                if let Some(fragment) = self.fragment(&selection) {
                    let corner = corner(&fragment);
                    self.paste_fragment(
                        &fragment,
                        [corner[0] + PASTE_SHIFT, corner[1] + PASTE_SHIFT],
                    );
                }
            }
            CanvasEdit::Paste(at) => self.paste(at),
            CanvasEdit::Undo => self.travel(true),
            CanvasEdit::Redo => self.travel(false),
        }
    }

    fn move_nodes(&mut self, moves: &[(rikka_widgets::node_canvas::NodeKey, Point)]) {
        let Some((_, graph)) = self.shown() else {
            return;
        };
        let scope = self.shader().scope();
        let Some(body) = graph.scope(&scope) else {
            return;
        };
        let edits: Vec<Edit> = moves
            .iter()
            .filter_map(|(key, at)| {
                Some(Edit::MoveNode {
                    scope: scope.clone(),
                    id: node_id(body, *key)?,
                    position: [at.x, at.y],
                })
            })
            .collect();
        self.edit_graph(None, |graph, library| batch(graph, library, edits.clone()));
    }

    fn reshape_comment(&mut self, key: FrameKey, at: Option<[f32; 2]>, size: Option<[f32; 2]>) {
        let Some(FrameId::Comment(id)) = self.frame(key) else {
            return;
        };
        let scope = self.shader().scope();
        self.edit_graph(None, |graph, library| {
            let mut editor = graph.edit(library, scope.clone());
            match (at, size) {
                (Some(at), _) => editor.move_comment(id, at),
                (None, Some(size)) => editor.resize_comment(id, size),
                (None, None) => Ok(Change {
                    value: (),
                    undo: Edit::Batch(Vec::new()),
                }),
            }
        });
    }

    fn connect(&mut self, from: PortAddr, to: PortAddr, replacing: Option<PortAddr>) {
        let Some((_, graph)) = self.shown() else {
            return;
        };
        let scope = self.shader().scope();
        let library = self.library();
        let (Some(from), Some(target)) = (
            resolve(graph, library, &scope, from),
            resolve(graph, library, &scope, to),
        ) else {
            return;
        };
        let mut edits = Vec::new();
        if let Some(detached) = replacing.filter(|detached| *detached != to)
            && let Some(old) = resolve(graph, library, &scope, detached)
        {
            edits.push(Edit::Link {
                scope: scope.clone(),
                to: PortRef::new(old.node, old.port),
                from: None,
            });
        }
        let connection = Connection {
            from: PortRef::new(from.node, from.port),
            to: PortRef::new(target.node, target.port),
        };
        let known = graph
            .scope(&scope)
            .is_some_and(|body| body.connections().contains(&connection));
        match (target.many, known) {
            (true, true) => {}
            (true, false) => edits.push(Edit::Attach {
                scope: scope.clone(),
                connection,
            }),
            (false, _) => edits.push(Edit::Link {
                scope: scope.clone(),
                to: connection.to,
                from: Some(connection.from),
            }),
        }
        if !edits.is_empty() {
            self.edit_graph(None, |graph, library| batch(graph, library, edits.clone()));
        }
    }

    fn disconnect(&mut self, input: PortAddr) {
        let Some((_, graph)) = self.shown() else {
            return;
        };
        let scope = self.shader().scope();
        let Some(target) = resolve(graph, self.library(), &scope, input) else {
            return;
        };
        self.edit_graph(None, |graph, library| {
            graph
                .edit(library, scope.clone())
                .disconnect(PortRef::new(target.node, target.port.clone()))
        });
    }

    pub(crate) fn detach(&mut self, from: PortRef, to: PortRef) {
        let scope = self.shader().scope();
        self.edit_graph(None, |graph, library| {
            graph
                .edit(library, scope.clone())
                .detach(from.clone(), to.clone())
        });
    }

    pub(crate) fn delete(&mut self, selection: &Selection) {
        let Some((_, graph)) = self.shown() else {
            return;
        };
        let scope = self.shader().scope();
        let library = self.library();
        let Some(body) = graph.scope(&scope) else {
            return;
        };
        let mut edits: Vec<Edit> = selection
            .links()
            .filter_map(|link| {
                let from = resolve(graph, library, &scope, link.from)?;
                let to = resolve(graph, library, &scope, link.to)?;
                let connection = Connection {
                    from: PortRef::new(from.node, from.port),
                    to: PortRef::new(to.node, to.port),
                };
                body.connections()
                    .contains(&connection)
                    .then(|| Edit::Detach {
                        scope: scope.clone(),
                        connection,
                    })
            })
            .collect();
        for key in selection.frames() {
            edits.push(match frame_id(body, key) {
                Some(FrameId::Group(id)) => Edit::RemoveGroup {
                    scope: scope.clone(),
                    id,
                },
                Some(FrameId::Comment(id)) => Edit::RemoveComment {
                    scope: scope.clone(),
                    id,
                },
                None => continue,
            });
        }
        edits.extend(
            selected_nodes(graph, &scope, selection)
                .into_iter()
                .map(|id| Edit::RemoveNode {
                    scope: scope.clone(),
                    id,
                }),
        );
        if edits.is_empty() {
            return;
        }
        if self
            .edit_graph(None, |graph, library| batch(graph, library, edits.clone()))
            .is_some()
        {
            self.select(Selection::default());
        }
    }

    fn fragment(&mut self, selection: &Selection) -> Option<Fragment> {
        let (_, graph) = self.shown()?;
        let scope = self.shader().scope();
        let nodes = selected_nodes(graph, &scope, selection);
        if nodes.is_empty() {
            return None;
        }
        match graph.copy(&scope, &nodes) {
            Ok(fragment) => Some(fragment),
            Err(error) => {
                self.fail(error);
                None
            }
        }
    }

    fn copy(&mut self, selection: &Selection) {
        if let Some(fragment) = self.fragment(selection) {
            self.shader_mut().clipboard = Some(fragment);
        }
    }

    pub(crate) fn paste(&mut self, at: Point) {
        let Some(fragment) = self.shader().clipboard.clone() else {
            return;
        };
        self.paste_fragment(&fragment, [at.x, at.y]);
    }

    fn paste_fragment(&mut self, fragment: &Fragment, at: [f32; 2]) {
        let corner = corner(fragment);
        let offset = [at[0] - corner[0], at[1] - corner[1]];
        let scope = self.shader().scope();
        if let Some(ids) = self.edit_graph(None, |graph, library| {
            graph.edit(library, scope.clone()).paste(fragment, offset)
        }) {
            self.select(Selection::of(
                ids.values().map(|id| Item::Node(node_key(*id))),
            ));
        }
    }

    pub(crate) fn add_catalog_node(&mut self, kind: NodeKind) {
        let menu = self.shader().menu;
        self.close_menu();
        let at = menu.map_or([0.0, 0.0], |menu| [menu.at.x, menu.at.y]);
        let scope = self.shader().scope();
        let dragged = menu.and_then(|menu| menu.from).and_then(|from| {
            let (_, graph) = self.shown()?;
            Some((from.side, resolve(graph, self.library(), &scope, from)?))
        });
        let added = self.edit_graph(None, |graph, library| {
            let mut undo = Vec::new();
            let added = graph
                .edit(library, scope.clone())
                .add_node(kind.clone(), at)?;
            undo.push(added.undo);
            let id = added.value;
            if let Some((side, end)) = &dragged {
                let ports = graph.ports(library, &scope, &kind)?;
                let pair =
                    match side {
                        PortSide::Output => ports
                            .inputs
                            .iter()
                            .find(|port| accepts(end.ty, port))
                            .map(|port| {
                                (
                                    PortRef::new(end.node, end.port.clone()),
                                    PortRef::new(id, port.name.clone()),
                                )
                            }),
                        PortSide::Input => ports
                            .outputs
                            .iter()
                            .find(|port| produces(port, end.ty))
                            .map(|port| {
                                (
                                    PortRef::new(id, port.name.clone()),
                                    PortRef::new(end.node, end.port.clone()),
                                )
                            }),
                    };
                if let Some((from, to)) = pair
                    && let Ok(linked) = graph.edit(library, scope.clone()).connect(from, to)
                {
                    undo.push(linked.undo);
                }
            }
            undo.reverse();
            Ok(Change {
                value: id,
                undo: Edit::Batch(undo),
            })
        });
        if let Some(id) = added {
            self.select_node(id);
        }
    }

    pub(crate) fn frame_selection(&mut self) {
        let nodes = self.selected_nodes();
        if nodes.is_empty() {
            return;
        }
        let scope = self.shader().scope();
        let title = tr!("animate.shader.frame_default").to_string();
        if let Some(id) = self.edit_graph(None, |graph, library| {
            graph.edit(library, scope.clone()).add_group(Group {
                title: title.clone(),
                nodes: nodes.clone(),
                color: None,
            })
        }) {
            self.select(Selection::of([Item::Frame(crate::canvas_data::frame_key(
                FrameId::Group(id),
            ))]));
        }
    }

    pub(crate) fn add_comment(&mut self, at: Point) {
        let scope = self.shader().scope();
        let text = tr!("animate.shader.comment_default").to_string();
        if let Some(id) = self.edit_graph(None, |graph, library| {
            graph.edit(library, scope.clone()).add_comment(Comment {
                text: text.clone(),
                position: [at.x, at.y],
                size: COMMENT_SIZE,
            })
        }) {
            self.select(Selection::of([Item::Frame(crate::canvas_data::frame_key(
                FrameId::Comment(id),
            ))]));
        }
    }

    pub(crate) fn rename_frame(&mut self, key: FrameKey, title: String) {
        let Some(frame) = self.frame(key) else {
            return;
        };
        let Some((_, graph)) = self.shown() else {
            return;
        };
        let scope = self.shader().scope();
        let Some(body) = graph.scope(&scope) else {
            return;
        };
        let edit = match frame {
            FrameId::Group(id) => {
                let Some(group) = body.groups().get(&id) else {
                    return;
                };
                if group.title == title {
                    return;
                }
                Edit::ReplaceGroup {
                    scope: scope.clone(),
                    id,
                    group: Group {
                        title,
                        ..group.clone()
                    },
                }
            }
            FrameId::Comment(id) => {
                let Some(comment) = body.comments().get(&id) else {
                    return;
                };
                if comment.text == title {
                    return;
                }
                Edit::ReplaceComment {
                    scope: scope.clone(),
                    id,
                    comment: Comment {
                        text: title,
                        ..comment.clone()
                    },
                }
            }
        };
        self.edit_graph(None, |graph, library| {
            batch(graph, library, vec![edit.clone()])
        });
    }

    pub(crate) fn arrange(&mut self) {
        let Some((_, graph)) = self.shown() else {
            return;
        };
        let mut arranged = graph.clone();
        arranged.arrange(self.library());
        let edits: Vec<Edit> = arranged
            .scopes()
            .flat_map(|(scope, body)| {
                let before = graph.scope(&scope);
                body.nodes()
                    .iter()
                    .filter(move |(id, node)| {
                        before
                            .and_then(|before| before.node(**id))
                            .is_some_and(|old| old.position != node.position)
                    })
                    .map(move |(id, node)| Edit::MoveNode {
                        scope: scope.clone(),
                        id: *id,
                        position: node.position,
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        if !edits.is_empty() {
            self.edit_graph(None, |graph, library| batch(graph, library, edits.clone()));
        }
    }
}

fn corner(fragment: &Fragment) -> [f32; 2] {
    fragment
        .nodes
        .values()
        .map(|node| node.position)
        .reduce(|one, two| [one[0].min(two[0]), one[1].min(two[1])])
        .unwrap_or([0.0, 0.0])
}
