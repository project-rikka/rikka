use crate::error::{Error, Result};
use crate::line_break::{Action, Ahead, Decision, LineBreaking};
use crate::rust::RustSource;

const AHEAD: u16 = 3;
const BEYOND_BASE: usize = 3;
const BMP: usize = 0x1_0000;
const BLOCK: usize = 64;

fn decision_code(decision: Option<Decision>) -> u8 {
    match decision {
        None => 0,
        Some(Decision::Allowed) => 1,
        Some(Decision::Mandatory) => 2,
    }
}

pub(crate) fn emit(
    breaking: &LineBreaking,
    sizes: &mut Vec<(&'static str, usize)>,
) -> Result<String> {
    let machine = &breaking.machine;
    let columns = machine.columns.len();
    if machine.states * columns >= 1 << 14 {
        return Err(Error::Data(format!(
            "the line break machine has {} states",
            machine.states
        )));
    }
    let first_complex = machine
        .columns
        .iter()
        .position(|column| column.complex)
        .unwrap_or(columns);
    if machine.columns[first_complex..]
        .iter()
        .any(|column| !column.complex)
    {
        return Err(Error::Data(
            "complex context columns do not follow every other column".into(),
        ));
    }
    let mut ahead_keys = Vec::new();
    let mut ahead_tables = Vec::new();
    let transitions: Vec<u16> = machine
        .transitions
        .iter()
        .enumerate()
        .map(|(index, &(action, target))| {
            let code = match action {
                Action::Decided(decision) => u16::from(decision_code(decision)),
                Action::Ahead(table) => {
                    ahead_keys.push(index as u32);
                    ahead_tables.push(table as u16);
                    AHEAD
                }
            };
            ((target * columns) as u16) << 2 | code
        })
        .collect();
    let after: Vec<u8> = machine
        .after_tables
        .iter()
        .flatten()
        .map(|&entry| match entry {
            Ahead::Decided(decision) => decision_code(decision),
            Ahead::Beyond(table) => (BEYOND_BASE + table) as u8,
        })
        .collect();
    let beyond: Vec<u8> = machine
        .beyond_tables
        .iter()
        .flatten()
        .map(|&decision| decision_code(decision))
        .collect();
    let ascii: Vec<u8> = breaking.columns[..128].to_vec();
    let (blocks, leaves) = bmp_blocks(&breaking.columns[..BMP]);

    let mut source = RustSource::default();
    source.constant("LINE_BREAK_COLUMN_COUNT", "usize", columns);
    source.constant("LINE_BREAK_START", "u16", machine.start * columns);
    source.constant("LINE_BREAK_FIRST_COMPLEX_COLUMN", "usize", first_complex);
    source.constant("LINE_BREAK_AHEAD", "u16", AHEAD);
    source.constant("LINE_BREAK_BEYOND_BASE", "u8", BEYOND_BASE);
    source.array("pub(crate) static LINE_BREAK_ASCII", "u8", &ascii);
    source.array("pub(crate) static LINE_BREAK_BMP_BLOCKS", "u16", &blocks);
    source.array("pub(crate) static LINE_BREAK_BMP_LEAVES", "u8", &leaves);
    source.array(
        "pub(crate) static LINE_BREAK_ATTACHING",
        "bool",
        machine.columns.iter().map(|column| column.attaches),
    );
    source.array(
        "pub(crate) static LINE_BREAK_ABSORBING",
        "bool",
        machine.columns.iter().map(|column| column.absorbs),
    );
    source.array(
        "pub(crate) static LINE_BREAK_TRANSITIONS",
        "u16",
        &transitions,
    );
    source.array(
        "pub(crate) static LINE_BREAK_AHEAD_KEYS",
        "u32",
        &ahead_keys,
    );
    source.array(
        "pub(crate) static LINE_BREAK_AHEAD_TABLES",
        "u16",
        &ahead_tables,
    );
    source.array("pub(crate) static LINE_BREAK_AFTER", "u8", &after);
    source.array("pub(crate) static LINE_BREAK_BEYOND", "u8", &beyond);
    sizes.push((
        "line break machine",
        transitions.len() * 2
            + ahead_keys.len() * 6
            + after.len()
            + beyond.len()
            + 128
            + blocks.len() * 2
            + leaves.len(),
    ));
    Ok(source.finish())
}

fn bmp_blocks(columns: &[u8]) -> (Vec<u16>, Vec<u8>) {
    let mut leaves: Vec<u8> = Vec::new();
    let mut known: std::collections::HashMap<&[u8], u16> = std::collections::HashMap::new();
    let blocks = columns
        .chunks(BLOCK)
        .map(|block| {
            *known.entry(block).or_insert_with(|| {
                leaves.extend_from_slice(block);
                (leaves.len() - BLOCK) as u16
            })
        })
        .collect();
    (blocks, leaves)
}
