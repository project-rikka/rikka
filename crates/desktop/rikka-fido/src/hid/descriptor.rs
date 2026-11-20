const FIDO_PAGE: u32 = 0xf1d0;
const CTAPHID_USAGE: u32 = 0x01;

const MAIN: u8 = 0;
const GLOBAL: u8 = 1;
const LOCAL: u8 = 2;

const INPUT: u8 = 0x8;
const OUTPUT: u8 = 0x9;
const COLLECTION: u8 = 0xa;
const END_COLLECTION: u8 = 0xc;

const USAGE_PAGE: u8 = 0x0;
const REPORT_SIZE: u8 = 0x7;
const REPORT_COUNT: u8 = 0x9;
const USAGE: u8 = 0x0;

const LONG_ITEM: u8 = 0xfe;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reports {
    pub(crate) input: usize,
    pub(crate) output: usize,
}

pub(crate) fn fido_reports(descriptor: &[u8]) -> Option<Reports> {
    let mut page = 0u32;
    let mut size = 0usize;
    let mut count = 0usize;
    let mut usages: Vec<u32> = Vec::new();
    let mut depth = 0usize;
    let mut fido_at: Option<usize> = None;
    let mut reports = Reports {
        input: 0,
        output: 0,
    };

    let mut at = 0;
    while at < descriptor.len() {
        let prefix = descriptor[at];

        if prefix == LONG_ITEM {
            let length = usize::from(*descriptor.get(at + 1)?);
            at += 3 + length;
            continue;
        }

        let length = match prefix & 0x03 {
            3 => 4,
            other => usize::from(other),
        };
        let data = descriptor.get(at + 1..at + 1 + length)?;
        at += 1 + length;

        let value = data
            .iter()
            .rev()
            .fold(0u32, |sum, byte| (sum << 8) | u32::from(*byte));
        let kind = (prefix >> 2) & 0x03;
        let tag = prefix >> 4;

        match (kind, tag) {
            (GLOBAL, USAGE_PAGE) => page = value,
            (GLOBAL, REPORT_SIZE) => size = value as usize,
            (GLOBAL, REPORT_COUNT) => count = value as usize,
            (LOCAL, USAGE) if length == 4 => usages.push(value),
            (LOCAL, USAGE) => usages.push((page << 16) | value),
            (MAIN, COLLECTION) => {
                if fido_at.is_none() && usages.contains(&((FIDO_PAGE << 16) | CTAPHID_USAGE)) {
                    fido_at = Some(depth);
                }
                depth += 1;
                usages.clear();
            }
            (MAIN, END_COLLECTION) => {
                depth = depth.saturating_sub(1);
                if fido_at == Some(depth) {
                    break;
                }
                usages.clear();
            }
            (MAIN, INPUT) if fido_at.is_some() => {
                reports.input = reports.input.max(size * count / 8);
                usages.clear();
            }
            (MAIN, OUTPUT) if fido_at.is_some() => {
                reports.output = reports.output.max(size * count / 8);
                usages.clear();
            }
            (MAIN, _) => usages.clear(),
            _ => {}
        }
    }

    (fido_at.is_some() && reports.input > 0 && reports.output > 0).then_some(reports)
}
