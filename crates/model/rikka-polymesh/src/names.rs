pub const MATERIAL_INDEX: &str = "material_index";
pub const SHARP_FACE: &str = "sharp_face";
pub const SHARP_EDGE: &str = "sharp_edge";
pub const UV_SEAM: &str = "uv_seam";
pub const CREASE_EDGE: &str = "crease_edge";
pub const CREASE_VERT: &str = "crease_vert";
pub const BEVEL_WEIGHT_EDGE: &str = "bevel_weight_edge";
pub const BEVEL_WEIGHT_VERT: &str = "bevel_weight_vert";
pub const SELECT_VERT: &str = ".select_vert";
pub const SELECT_EDGE: &str = ".select_edge";
pub const SELECT_POLY: &str = ".select_poly";
pub const HIDE_VERT: &str = ".hide_vert";
pub const HIDE_EDGE: &str = ".hide_edge";
pub const HIDE_POLY: &str = ".hide_poly";
pub const CUSTOM_NORMAL: &str = "custom_normal";
pub const UV_SELECT_VERT: &str = ".uv_select_vert";
pub const UV_SELECT_EDGE: &str = ".uv_select_edge";
pub const UV_SELECT_FACE: &str = ".uv_select_face";
pub const UV_PIN_PREFIX: &str = ".pn.";

#[must_use]
pub fn uv_pin_name(uv_map: &str) -> String {
    format!("{UV_PIN_PREFIX}{uv_map}")
}
