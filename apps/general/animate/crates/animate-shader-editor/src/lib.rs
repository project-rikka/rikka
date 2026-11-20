mod actions;
mod canvas_data;
mod catalog;
mod sockets;
mod state;
mod traits;
mod views;

pub use canvas_data::{
    FrameId, category_color, frame_id, frame_key, node_graph, node_id, node_key, port_color,
};
pub use catalog::{CATEGORIES, CatalogLine, Dragged, accepts, catalog_lines, produces};
pub use sockets::{
    Fallback, Field, LinkSource, ParameterRow, SocketRow, SocketState, parameter_rows, socket_rows,
};
pub use state::{ShaderEditor, Tab};
pub use traits::{
    AssetFilter, Desk, KeyState, LookValues, MaterialGraphs, Reach, ShaderCompiling, ShaderEditing,
    SlotFigure, SlotMaterial, Workshop,
};
pub use views::{node_editor, node_properties};

pub static CATALOGS: &[(&str, &str)] = rikka_i18n::catalogs!();
