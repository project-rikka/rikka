const ONLINE: &str = "/sys/devices/system/cpu/online";

pub(crate) fn online() -> usize {
    std::fs::read_to_string(ONLINE)
        .ok()
        .and_then(|list| count(&list))
        .or_else(|| std::thread::available_parallelism().ok().map(usize::from))
        .unwrap_or(1)
        .max(1)
}

fn count(list: &str) -> Option<usize> {
    list.trim()
        .split(',')
        .map(|range| match range.split_once('-') {
            Some((first, last)) => {
                let first: usize = first.parse().ok()?;
                let last: usize = last.parse().ok()?;
                last.checked_sub(first).map(|span| span + 1)
            }
            None => range.parse::<usize>().ok().map(|_| 1),
        })
        .sum()
}
