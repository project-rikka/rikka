use rikka_slang::TextureKind;

use crate::library::NodeSpec;
use crate::nodes::{Function, number, texture};
use crate::ops::Category;

pub(super) fn specs() -> Vec<NodeSpec> {
    vec![
        Function::new(
            "texture.bound",
            "Texture bound",
            "1 when a texture is bound to the input, 0 when it is empty (an empty texture samples white), so a graph can fall back to its own values for maps a model lacks.",
        )
        .category(Category::Texture)
        .value(
            &[texture("texture", TextureKind::Data)],
            number("value"),
            "({texture}.bound() ? 1.0 : 0.0)",
        ),
    ]
}
