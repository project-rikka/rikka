pub(crate) fn toggled(items: &[String], item: &str, on: bool) -> Vec<String> {
    let mut toggled: Vec<String> = items
        .iter()
        .filter(|existing| *existing != item)
        .cloned()
        .collect();
    if on {
        toggled.push(item.to_owned());
    }
    toggled
}
