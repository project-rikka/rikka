#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContentRange {
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) total: Option<u64>,
}

pub(crate) fn parse(value: &str) -> Option<ContentRange> {
    let spec = value.trim().strip_prefix("bytes")?.trim_start();
    let (range, total) = spec.split_once('/')?;
    let (start, end) = range.split_once('-')?;

    let range = ContentRange {
        start: start.trim().parse().ok()?,
        end: end.trim().parse().ok()?,
        total: total.trim().parse().ok(),
    };

    (range.start <= range.end).then_some(range)
}
