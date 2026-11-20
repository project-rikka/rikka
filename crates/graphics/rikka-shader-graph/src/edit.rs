use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::check::{self, Report};
use crate::error::{At, GraphError, Problem};
use crate::graph::{
    Body, Comment, CommentId, Connection, Function, Graph, GraphKind, Group, GroupId, Node, NodeId,
    PortRef, Scope,
};
use crate::library::Library;
use crate::node::NodeKind;
use crate::parameter::ParameterDecl;
use crate::types::SocketValue;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edit {
    Batch(Vec<Edit>),
    SetKind(GraphKind),
    InsertParameter {
        at: usize,
        parameter: ParameterDecl,
    },
    RemoveParameter {
        name: String,
    },
    ReplaceParameter {
        parameter: ParameterDecl,
    },
    RenameParameter {
        from: String,
        to: String,
    },
    InsertFunction {
        name: String,
        function: Function,
    },
    RemoveFunction {
        name: String,
    },
    ReplaceFunction {
        name: String,
        function: Function,
    },
    InsertNode {
        scope: Scope,
        id: NodeId,
        node: Node,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        connections: Vec<Connection>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        groups: Vec<GroupId>,
    },
    RemoveNode {
        scope: Scope,
        id: NodeId,
    },
    SetNodeKind {
        scope: Scope,
        id: NodeId,
        kind: NodeKind,
    },
    MoveNode {
        scope: Scope,
        id: NodeId,
        position: [f32; 2],
    },
    SetInput {
        scope: Scope,
        id: NodeId,
        port: String,
        value: Option<SocketValue>,
    },
    Link {
        scope: Scope,
        to: PortRef,
        from: Option<PortRef>,
    },
    Attach {
        scope: Scope,
        connection: Connection,
    },
    Detach {
        scope: Scope,
        connection: Connection,
    },
    InsertGroup {
        scope: Scope,
        id: GroupId,
        group: Group,
    },
    RemoveGroup {
        scope: Scope,
        id: GroupId,
    },
    ReplaceGroup {
        scope: Scope,
        id: GroupId,
        group: Group,
    },
    InsertComment {
        scope: Scope,
        id: CommentId,
        comment: Comment,
    },
    RemoveComment {
        scope: Scope,
        id: CommentId,
    },
    ReplaceComment {
        scope: Scope,
        id: CommentId,
        comment: Comment,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Change<T> {
    pub value: T,
    pub undo: Edit,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fragment {
    pub nodes: BTreeMap<NodeId, Node>,
    #[serde(default)]
    pub connections: BTreeSet<Connection>,
}

fn fail(at: At, problem: Problem) -> GraphError {
    GraphError::new(at, problem)
}

fn missing_scope(scope: &Scope) -> GraphError {
    let problem = match scope {
        Scope::Main => Problem::UnknownFunction(String::new()),
        Scope::Function(name) => Problem::UnknownFunction(name.clone()),
    };
    fail(At::graph(), problem)
}

fn swap<T>(slot: &mut T, value: T) -> T {
    std::mem::replace(slot, value)
}

impl Graph {
    pub fn validate(&self, library: &Library) -> Report {
        check::validate(self, library)
    }

    pub fn apply(&mut self, library: &Library, edit: Edit) -> Result<Edit, GraphError> {
        let before = self.validate(library);
        let saved = self.clone();
        let undo = match self.raw(edit) {
            Ok(undo) => undo,
            Err(error) => {
                *self = saved;
                return Err(error);
            }
        };
        let after = self.validate(library);
        if let Some(error) = after
            .errors()
            .iter()
            .find(|error| !error.problem.is_incomplete() && !before.errors().contains(error))
        {
            let error = error.clone();
            *self = saved;
            return Err(error);
        }
        Ok(undo)
    }

    fn change<T>(
        &mut self,
        library: &Library,
        edit: Edit,
        value: T,
    ) -> Result<Change<T>, GraphError> {
        Ok(Change {
            undo: self.apply(library, edit)?,
            value,
        })
    }

    pub fn edit<'a>(&'a mut self, library: &'a Library, scope: Scope) -> Editor<'a> {
        Editor {
            graph: self,
            library,
            scope,
        }
    }

    pub fn set_kind(
        &mut self,
        library: &Library,
        kind: GraphKind,
    ) -> Result<Change<()>, GraphError> {
        self.change(library, Edit::SetKind(kind), ())
    }

    pub fn add_parameter(
        &mut self,
        library: &Library,
        parameter: ParameterDecl,
    ) -> Result<Change<()>, GraphError> {
        let at = self.parameters.len();
        self.change(library, Edit::InsertParameter { at, parameter }, ())
    }

    pub fn set_parameter(
        &mut self,
        library: &Library,
        parameter: ParameterDecl,
    ) -> Result<Change<()>, GraphError> {
        self.change(library, Edit::ReplaceParameter { parameter }, ())
    }

    pub fn rename_parameter(
        &mut self,
        library: &Library,
        from: &str,
        to: &str,
    ) -> Result<Change<()>, GraphError> {
        self.change(
            library,
            Edit::RenameParameter {
                from: from.to_owned(),
                to: to.to_owned(),
            },
            (),
        )
    }

    pub fn remove_parameter(
        &mut self,
        library: &Library,
        name: &str,
    ) -> Result<Change<()>, GraphError> {
        self.change(
            library,
            Edit::RemoveParameter {
                name: name.to_owned(),
            },
            (),
        )
    }

    pub fn add_function(
        &mut self,
        library: &Library,
        name: &str,
        function: Function,
    ) -> Result<Change<()>, GraphError> {
        self.change(
            library,
            Edit::InsertFunction {
                name: name.to_owned(),
                function,
            },
            (),
        )
    }

    pub fn set_function(
        &mut self,
        library: &Library,
        name: &str,
        function: Function,
    ) -> Result<Change<()>, GraphError> {
        self.change(
            library,
            Edit::ReplaceFunction {
                name: name.to_owned(),
                function,
            },
            (),
        )
    }

    pub fn remove_function(
        &mut self,
        library: &Library,
        name: &str,
    ) -> Result<Change<()>, GraphError> {
        self.change(
            library,
            Edit::RemoveFunction {
                name: name.to_owned(),
            },
            (),
        )
    }

    pub fn copy(&self, scope: &Scope, nodes: &BTreeSet<NodeId>) -> Result<Fragment, GraphError> {
        let body = self.scope(scope).ok_or_else(|| missing_scope(scope))?;
        if let Some(missing) = nodes.iter().find(|id| !body.nodes.contains_key(id)) {
            return Err(fail(
                At::node(scope, *missing),
                Problem::UnknownNode(*missing),
            ));
        }
        Ok(Fragment {
            nodes: body
                .nodes
                .iter()
                .filter(|(id, _)| nodes.contains(id))
                .map(|(id, node)| (*id, node.clone()))
                .collect(),
            connections: body
                .connections
                .iter()
                .filter(|connection| {
                    nodes.contains(&connection.from.node) && nodes.contains(&connection.to.node)
                })
                .cloned()
                .collect(),
        })
    }

    fn body_mut(&mut self, scope: &Scope) -> Result<&mut Body, GraphError> {
        self.scope_mut(scope).ok_or_else(|| missing_scope(scope))
    }

    fn raw(&mut self, edit: Edit) -> Result<Edit, GraphError> {
        Ok(match edit {
            Edit::Batch(edits) => {
                let mut undo = Vec::with_capacity(edits.len());
                for edit in edits {
                    undo.push(self.raw(edit)?);
                }
                undo.reverse();
                Edit::Batch(undo)
            }
            Edit::SetKind(kind) => Edit::SetKind(swap(&mut self.kind, kind)),
            Edit::InsertParameter { at, parameter } => {
                let at = at.min(self.parameters.len());
                let name = parameter.name.clone();
                self.parameters.insert(at, parameter);
                Edit::RemoveParameter { name }
            }
            Edit::RemoveParameter { name } => {
                let at = self.parameter_index(&name)?;
                Edit::InsertParameter {
                    at,
                    parameter: self.parameters.remove(at),
                }
            }
            Edit::ReplaceParameter { parameter } => {
                let at = self.parameter_index(&parameter.name)?;
                Edit::ReplaceParameter {
                    parameter: swap(&mut self.parameters[at], parameter),
                }
            }
            Edit::RenameParameter { from, to } => {
                let at = self.parameter_index(&from)?;
                if self.parameter(&to).is_some() {
                    return Err(fail(At::graph(), Problem::Taken(format!("parameter {to}"))));
                }
                self.parameters[at].name = to.clone();
                for body in self.bodies_mut() {
                    for node in body.nodes.values_mut() {
                        if matches!(&node.kind, NodeKind::Parameter(name) if *name == from) {
                            node.kind = NodeKind::Parameter(to.clone());
                        }
                    }
                }
                Edit::RenameParameter { from: to, to: from }
            }
            Edit::InsertFunction { name, function } => {
                if self.functions.contains_key(&name) {
                    return Err(fail(
                        At::scope(&Scope::Function(name.clone())),
                        Problem::Duplicate(name),
                    ));
                }
                self.functions.insert(name.clone(), function);
                Edit::RemoveFunction { name }
            }
            Edit::RemoveFunction { name } => {
                let function = self
                    .functions
                    .remove(&name)
                    .ok_or_else(|| fail(At::graph(), Problem::UnknownFunction(name.clone())))?;
                Edit::InsertFunction { name, function }
            }
            Edit::ReplaceFunction { name, function } => {
                let slot = self
                    .functions
                    .get_mut(&name)
                    .ok_or_else(|| fail(At::graph(), Problem::UnknownFunction(name.clone())))?;
                Edit::ReplaceFunction {
                    function: swap(slot, function),
                    name,
                }
            }
            Edit::InsertNode {
                scope,
                id,
                node,
                connections,
                groups,
            } => {
                let body = self.body_mut(&scope)?;
                if body.nodes.contains_key(&id) {
                    return Err(fail(
                        At::node(&scope, id),
                        Problem::Taken(format!("node {id}")),
                    ));
                }
                for group in &groups {
                    body.groups
                        .get_mut(group)
                        .ok_or_else(|| fail(At::scope(&scope), Problem::UnknownGroup(*group)))?
                        .nodes
                        .insert(id);
                }
                body.nodes.insert(id, node);
                body.connections.extend(connections);
                Edit::RemoveNode { scope, id }
            }
            Edit::RemoveNode { scope, id } => {
                let body = self.body_mut(&scope)?;
                let node = body
                    .nodes
                    .remove(&id)
                    .ok_or_else(|| fail(At::node(&scope, id), Problem::UnknownNode(id)))?;
                let (connections, kept): (BTreeSet<Connection>, BTreeSet<Connection>) =
                    std::mem::take(&mut body.connections)
                        .into_iter()
                        .partition(|connection| {
                            connection.from.node == id || connection.to.node == id
                        });
                body.connections = kept;
                let groups = body
                    .groups
                    .iter_mut()
                    .filter_map(|(group, members)| members.nodes.remove(&id).then_some(*group))
                    .collect();
                Edit::InsertNode {
                    scope,
                    id,
                    node,
                    connections: connections.into_iter().collect(),
                    groups,
                }
            }
            Edit::SetNodeKind { scope, id, kind } => {
                let node = self.node_mut(&scope, id)?;
                Edit::SetNodeKind {
                    kind: swap(&mut node.kind, kind),
                    scope,
                    id,
                }
            }
            Edit::MoveNode {
                scope,
                id,
                position,
            } => {
                let node = self.node_mut(&scope, id)?;
                Edit::MoveNode {
                    position: swap(&mut node.position, position),
                    scope,
                    id,
                }
            }
            Edit::SetInput {
                scope,
                id,
                port,
                value,
            } => {
                let node = self.node_mut(&scope, id)?;
                let old = match value {
                    Some(value) => node.inputs.insert(port.clone(), value),
                    None => node.inputs.remove(&port),
                };
                Edit::SetInput {
                    scope,
                    id,
                    port,
                    value: old,
                }
            }
            Edit::Link { scope, to, from } => {
                let body = self.body_mut(&scope)?;
                let old = body
                    .connections
                    .iter()
                    .find(|connection| connection.to == to)
                    .cloned();
                if let Some(old) = &old {
                    body.connections.remove(old);
                }
                if let Some(from) = from {
                    body.connections.insert(Connection {
                        from,
                        to: to.clone(),
                    });
                }
                Edit::Link {
                    scope,
                    to,
                    from: old.map(|connection| connection.from),
                }
            }
            Edit::Attach { scope, connection } => {
                let body = self.body_mut(&scope)?;
                if !body.connections.insert(connection.clone()) {
                    return Err(fail(
                        At::port(&scope, connection.to.node, &connection.to.port),
                        Problem::AlreadyConnected,
                    ));
                }
                Edit::Detach { scope, connection }
            }
            Edit::Detach { scope, connection } => {
                let body = self.body_mut(&scope)?;
                if !body.connections.remove(&connection) {
                    return Err(fail(
                        At::port(&scope, connection.to.node, &connection.to.port),
                        Problem::UnknownInput(connection.to.port.clone()),
                    ));
                }
                Edit::Attach { scope, connection }
            }
            Edit::InsertGroup { scope, id, group } => {
                let body = self.body_mut(&scope)?;
                if body.groups.contains_key(&id) {
                    return Err(fail(
                        At::scope(&scope),
                        Problem::Taken(format!("group {id}")),
                    ));
                }
                body.groups.insert(id, group);
                Edit::RemoveGroup { scope, id }
            }
            Edit::RemoveGroup { scope, id } => {
                let body = self.body_mut(&scope)?;
                let group = body
                    .groups
                    .remove(&id)
                    .ok_or_else(|| fail(At::scope(&scope), Problem::UnknownGroup(id)))?;
                Edit::InsertGroup { scope, id, group }
            }
            Edit::ReplaceGroup { scope, id, group } => {
                let body = self.body_mut(&scope)?;
                let slot = body
                    .groups
                    .get_mut(&id)
                    .ok_or_else(|| fail(At::scope(&scope), Problem::UnknownGroup(id)))?;
                Edit::ReplaceGroup {
                    group: swap(slot, group),
                    scope,
                    id,
                }
            }
            Edit::InsertComment { scope, id, comment } => {
                let body = self.body_mut(&scope)?;
                if body.comments.contains_key(&id) {
                    return Err(fail(
                        At::scope(&scope),
                        Problem::Taken(format!("comment {id}")),
                    ));
                }
                body.comments.insert(id, comment);
                Edit::RemoveComment { scope, id }
            }
            Edit::RemoveComment { scope, id } => {
                let body = self.body_mut(&scope)?;
                let comment = body
                    .comments
                    .remove(&id)
                    .ok_or_else(|| fail(At::scope(&scope), Problem::UnknownComment(id)))?;
                Edit::InsertComment { scope, id, comment }
            }
            Edit::ReplaceComment { scope, id, comment } => {
                let body = self.body_mut(&scope)?;
                let slot = body
                    .comments
                    .get_mut(&id)
                    .ok_or_else(|| fail(At::scope(&scope), Problem::UnknownComment(id)))?;
                Edit::ReplaceComment {
                    comment: swap(slot, comment),
                    scope,
                    id,
                }
            }
        })
    }

    fn parameter_index(&self, name: &str) -> Result<usize, GraphError> {
        self.parameters
            .iter()
            .position(|parameter| parameter.name == name)
            .ok_or_else(|| fail(At::graph(), Problem::UnknownParameter(name.to_owned())))
    }

    fn node_mut(&mut self, scope: &Scope, id: NodeId) -> Result<&mut Node, GraphError> {
        self.body_mut(scope)?
            .nodes
            .get_mut(&id)
            .ok_or_else(|| fail(At::node(scope, id), Problem::UnknownNode(id)))
    }

    fn bodies_mut(&mut self) -> impl Iterator<Item = &mut Body> {
        std::iter::once(&mut self.body).chain(
            self.functions
                .values_mut()
                .map(|function| &mut function.body),
        )
    }
}

#[derive(Debug)]
pub struct Editor<'a> {
    graph: &'a mut Graph,
    library: &'a Library,
    scope: Scope,
}

impl Editor<'_> {
    fn body(&self) -> Result<&Body, GraphError> {
        self.graph
            .scope(&self.scope)
            .ok_or_else(|| missing_scope(&self.scope))
    }

    pub fn add_node(
        &mut self,
        kind: NodeKind,
        position: [f32; 2],
    ) -> Result<Change<NodeId>, GraphError> {
        let id = self.body()?.fresh();
        let edit = Edit::InsertNode {
            scope: self.scope.clone(),
            id,
            node: Node::new(kind, position),
            connections: Vec::new(),
            groups: Vec::new(),
        };
        self.graph.change(self.library, edit, id)
    }

    pub fn remove_node(&mut self, id: NodeId) -> Result<Change<()>, GraphError> {
        let scope = self.scope.clone();
        self.graph
            .change(self.library, Edit::RemoveNode { scope, id }, ())
    }

    pub fn move_node(&mut self, id: NodeId, position: [f32; 2]) -> Result<Change<()>, GraphError> {
        let scope = self.scope.clone();
        self.graph.change(
            self.library,
            Edit::MoveNode {
                scope,
                id,
                position,
            },
            (),
        )
    }

    pub fn set_kind(&mut self, id: NodeId, kind: NodeKind) -> Result<Change<()>, GraphError> {
        let scope = self.scope.clone();
        self.graph
            .change(self.library, Edit::SetNodeKind { scope, id, kind }, ())
    }

    pub fn set_input(
        &mut self,
        id: NodeId,
        port: &str,
        value: Option<SocketValue>,
    ) -> Result<Change<()>, GraphError> {
        let scope = self.scope.clone();
        self.graph.change(
            self.library,
            Edit::SetInput {
                scope,
                id,
                port: port.to_owned(),
                value,
            },
            (),
        )
    }

    fn takes_many(&self, to: &PortRef) -> bool {
        self.body()
            .ok()
            .and_then(|body| body.node(to.node))
            .and_then(|node| self.graph.ports(self.library, &self.scope, &node.kind).ok())
            .and_then(|ports| ports.input(&to.port).map(|port| port.many))
            .unwrap_or(false)
    }

    pub fn connect(&mut self, from: PortRef, to: PortRef) -> Result<Change<()>, GraphError> {
        let scope = self.scope.clone();
        let edit = match self.takes_many(&to) {
            true => Edit::Attach {
                scope,
                connection: Connection { from, to },
            },
            false => Edit::Link {
                scope,
                to,
                from: Some(from),
            },
        };
        self.graph.change(self.library, edit, ())
    }

    pub fn disconnect(&mut self, to: PortRef) -> Result<Change<()>, GraphError> {
        let scope = self.scope.clone();
        let attached: Vec<Edit> = self
            .body()?
            .sources(&to)
            .map(|from| Edit::Detach {
                scope: scope.clone(),
                connection: Connection {
                    from: from.clone(),
                    to: to.clone(),
                },
            })
            .collect();
        self.graph.change(self.library, Edit::Batch(attached), ())
    }

    pub fn detach(&mut self, from: PortRef, to: PortRef) -> Result<Change<()>, GraphError> {
        let scope = self.scope.clone();
        self.graph.change(
            self.library,
            Edit::Detach {
                scope,
                connection: Connection { from, to },
            },
            (),
        )
    }

    pub fn add_group(&mut self, group: Group) -> Result<Change<GroupId>, GraphError> {
        let id = self.body()?.fresh_group();
        let scope = self.scope.clone();
        self.graph
            .change(self.library, Edit::InsertGroup { scope, id, group }, id)
    }

    pub fn set_group(&mut self, id: GroupId, group: Group) -> Result<Change<()>, GraphError> {
        let scope = self.scope.clone();
        self.graph
            .change(self.library, Edit::ReplaceGroup { scope, id, group }, ())
    }

    pub fn group_nodes(
        &mut self,
        id: GroupId,
        nodes: BTreeSet<NodeId>,
    ) -> Result<Change<()>, GraphError> {
        let group = self.group(id)?;
        self.set_group(id, Group { nodes, ..group })
    }

    pub fn remove_group(&mut self, id: GroupId) -> Result<Change<()>, GraphError> {
        let scope = self.scope.clone();
        self.graph
            .change(self.library, Edit::RemoveGroup { scope, id }, ())
    }

    fn group(&self, id: GroupId) -> Result<Group, GraphError> {
        self.body()?
            .groups
            .get(&id)
            .cloned()
            .ok_or_else(|| fail(At::scope(&self.scope), Problem::UnknownGroup(id)))
    }

    pub fn add_comment(&mut self, comment: Comment) -> Result<Change<CommentId>, GraphError> {
        let id = self.body()?.fresh_comment();
        let scope = self.scope.clone();
        self.graph
            .change(self.library, Edit::InsertComment { scope, id, comment }, id)
    }

    pub fn set_comment(
        &mut self,
        id: CommentId,
        comment: Comment,
    ) -> Result<Change<()>, GraphError> {
        let scope = self.scope.clone();
        self.graph.change(
            self.library,
            Edit::ReplaceComment { scope, id, comment },
            (),
        )
    }

    pub fn move_comment(
        &mut self,
        id: CommentId,
        position: [f32; 2],
    ) -> Result<Change<()>, GraphError> {
        let comment = self.comment(id)?;
        self.set_comment(
            id,
            Comment {
                position,
                ..comment
            },
        )
    }

    pub fn resize_comment(
        &mut self,
        id: CommentId,
        size: [f32; 2],
    ) -> Result<Change<()>, GraphError> {
        let comment = self.comment(id)?;
        self.set_comment(id, Comment { size, ..comment })
    }

    pub fn remove_comment(&mut self, id: CommentId) -> Result<Change<()>, GraphError> {
        let scope = self.scope.clone();
        self.graph
            .change(self.library, Edit::RemoveComment { scope, id }, ())
    }

    fn comment(&self, id: CommentId) -> Result<Comment, GraphError> {
        self.body()?
            .comments
            .get(&id)
            .cloned()
            .ok_or_else(|| fail(At::scope(&self.scope), Problem::UnknownComment(id)))
    }

    pub fn paste(
        &mut self,
        fragment: &Fragment,
        offset: [f32; 2],
    ) -> Result<Change<BTreeMap<NodeId, NodeId>>, GraphError> {
        let start = self.body()?.fresh().0;
        let ids: BTreeMap<NodeId, NodeId> = fragment
            .nodes
            .keys()
            .zip(start..)
            .map(|(old, new)| (*old, NodeId(new)))
            .collect();
        let mut edits: Vec<Edit> = fragment
            .nodes
            .iter()
            .map(|(old, node)| Edit::InsertNode {
                scope: self.scope.clone(),
                id: ids[old],
                node: Node {
                    position: [node.position[0] + offset[0], node.position[1] + offset[1]],
                    ..node.clone()
                },
                connections: Vec::new(),
                groups: Vec::new(),
            })
            .collect();
        for connection in &fragment.connections {
            let (Some(from), Some(to)) =
                (ids.get(&connection.from.node), ids.get(&connection.to.node))
            else {
                continue;
            };
            edits.push(Edit::Attach {
                scope: self.scope.clone(),
                connection: Connection {
                    from: PortRef::new(*from, connection.from.port.clone()),
                    to: PortRef::new(*to, connection.to.port.clone()),
                },
            });
        }
        self.graph.change(self.library, Edit::Batch(edits), ids)
    }
}
