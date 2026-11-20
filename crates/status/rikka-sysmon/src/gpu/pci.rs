const DATABASES: [&str; 2] = [
    "/run/current-system/sw/share/hwdata/pci.ids",
    "/run/current-system/sw/share/pci.ids",
];

pub(crate) fn name(vendor: u16, device: u16) -> String {
    lookup(vendor, device).unwrap_or_else(|| format!("{vendor:04x}:{device:04x}"))
}

fn lookup(vendor: u16, device: u16) -> Option<String> {
    let text = DATABASES
        .into_iter()
        .find_map(|path| std::fs::read_to_string(path).ok())?;
    let vendor = format!("{vendor:04x}");
    let device = format!("{device:04x}");

    let mut inside = false;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if !line.starts_with('\t') {
            if inside {
                return None;
            }
            inside = line.starts_with(&vendor);
            continue;
        }
        if !inside || line.starts_with("\t\t") {
            continue;
        }
        let entry = line.trim_start();
        if let Some(name) = entry.strip_prefix(&device) {
            return Some(name.trim().to_owned());
        }
    }
    None
}
