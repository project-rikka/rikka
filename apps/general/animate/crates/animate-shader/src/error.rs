#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Look(#[from] animate_look::Error),
    #[error(transparent)]
    Built(#[from] rikka_shader_graph::Error),
    #[error(transparent)]
    Graph(#[from] rikka_shader_graph::GraphError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
