#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Graph(#[from] rikka_shader_graph::Error),
    #[error(transparent)]
    Renderer(#[from] rikka_renderer_3d::Error),
}
