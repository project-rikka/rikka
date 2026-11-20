use super::{BUCKETS, RadixKey, digit, passes};

pub(super) fn sort<T: Copy, K: RadixKey>(
    counts: &mut Vec<u32>,
    items: &mut [T],
    scratch: &mut [T],
    key: &impl Fn(&T) -> K,
) {
    let passes = passes::<K>();
    counts.clear();
    counts.resize(passes * BUCKETS, 0);
    let (histograms, _) = counts.as_chunks_mut::<BUCKETS>();
    for item in items.iter() {
        let bits = key(item).radix_bits();
        for (pass, histogram) in histograms.iter_mut().enumerate() {
            histogram[digit(bits, pass)] += 1;
        }
    }
    let len = items.len();
    let mut sorted_in_scratch = false;
    for (pass, histogram) in histograms.iter_mut().enumerate() {
        if histogram.iter().any(|&count| count as usize == len) {
            continue;
        }
        let mut running = 0;
        for count in histogram.iter_mut() {
            running += std::mem::replace(count, running);
        }
        if sorted_in_scratch {
            scatter(scratch, items, histogram, pass, key);
        } else {
            scatter(items, scratch, histogram, pass, key);
        }
        sorted_in_scratch = !sorted_in_scratch;
    }
    if sorted_in_scratch {
        items.copy_from_slice(scratch);
    }
}

#[inline]
fn scatter<T: Copy, K: RadixKey>(
    source: &[T],
    target: &mut [T],
    offsets: &mut [u32],
    pass: usize,
    key: &impl Fn(&T) -> K,
) {
    for item in source {
        let offset = &mut offsets[digit(key(item).radix_bits(), pass)];
        target[*offset as usize] = *item;
        *offset += 1;
    }
}
