use std::collections::BTreeSet;

const PREFIX: &str = "mcp";
const JOIN: &str = "__";

const LONGEST: usize = 128;

const MARK: usize = 8;

pub(crate) fn label(server: &str) -> String {
    plain(server)
}

pub(crate) fn named(label: &str, tool: &str) -> String {
    trimmed(format!("{PREFIX}{JOIN}{label}{JOIN}{}", plain(tool)))
}

pub(crate) fn unique(base_name: String, used_names: &mut BTreeSet<String>) -> String {
    let mut name = base_name.clone();
    let mut nth = 1;
    while !used_names.insert(name.clone()) {
        nth += 1;
        name = trimmed(format!("{base_name}_{nth}"));
    }
    name
}

fn plain(text: &str) -> String {
    text.chars()
        .map(
            |letter| match letter.is_ascii_alphanumeric() || letter == '_' || letter == '-' {
                true => letter,
                false => '_',
            },
        )
        .collect()
}

fn trimmed(name: String) -> String {
    if name.len() <= LONGEST {
        return name;
    }
    let mark = marked(&name);
    format!("{}_{mark}", &name[..LONGEST - MARK - 1])
}

fn marked(name: &str) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = OFFSET;
    for byte in name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")[..MARK].to_owned()
}
