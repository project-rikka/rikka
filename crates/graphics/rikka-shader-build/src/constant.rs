pub(crate) fn constant(file: &str) -> String {
    file.replace(['.', '-'], "_").to_uppercase()
}
