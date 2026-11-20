use rikka_settings::session::color_picker::Format;

use crate::template::Template;

pub const NEW_FORMAT: &str = "new Color (R = %Re, G = %Gr, B = %Bl)";

#[must_use]
pub fn template_named(formats: &[Format], name: &str) -> Template {
    let source = formats
        .iter()
        .find(|format| format.name == name)
        .map_or("", |format| format.format.as_str());
    Template::parse(source)
}
