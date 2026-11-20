use crate::graph_ref::GraphRef;

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ValueError {
    #[error("{name} takes a {expected}, not a {given}")]
    Mismatch {
        name: String,
        expected: &'static str,
        given: &'static str,
    },
    #[error("{name} has no option `{option}`; the options are {options}")]
    UnknownOption {
        name: String,
        option: String,
        options: String,
    },
    #[error("{name} {value} is outside {low} to {high}")]
    OutOfRange {
        name: String,
        value: f32,
        low: f32,
        high: f32,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Value(#[from] ValueError),
    #[error("failed to build the look nodes: {0}")]
    Library(#[source] Box<rikka_shader_compile::Error>),
    #[error("{graph}: {source}")]
    Graph {
        graph: GraphRef,
        source: rikka_shader_graph::Error,
    },
    #[error("{graph}: {source}")]
    Edit {
        graph: GraphRef,
        source: Box<rikka_shader_graph::GraphError>,
    },
    #[error("{graph} is a {found} graph, not a {expected} graph")]
    Kind {
        graph: GraphRef,
        expected: &'static str,
        found: &'static str,
    },
    #[error("{0:?} is not a valid material name")]
    InvalidKey(String),
    #[error("there is no {0}")]
    Missing(GraphRef),
    #[error("{0} already exists")]
    Taken(GraphRef),
    #[error("{node} {port} does not hold {expected}")]
    Record {
        node: String,
        port: String,
        expected: &'static str,
    },
    #[error("light {}: {reason}", light + 1)]
    Light { light: usize, reason: String },
    #[error("key on {target}: {reason}")]
    Key { target: String, reason: String },
    #[error("duplicate {target} key at frame {frame}")]
    DuplicateKey { target: String, frame: u32 },
    #[error("material {material} of {figure} is declared twice")]
    DuplicateSlot { figure: String, material: u32 },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
