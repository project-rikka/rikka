pub(crate) fn compile_name(key: &str) -> String {
    let mut name = String::from("look_");
    name.extend(key.chars().map(|ch| match ch.is_ascii_alphanumeric() {
        true => ch.to_ascii_lowercase(),
        false => '_',
    }));
    name
}
