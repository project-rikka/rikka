use std::time::{Duration, Instant};

use rikka_compositor_recency::{FOCUS_DWELL, Recency};

fn order(recency: &Recency<u32>) -> Vec<u32> {
    recency.order().collect()
}

#[test]
fn focus_order_is_most_recent_first() {
    let start = Instant::now();
    let mut recency = Recency::default();
    for (step, window) in [1, 2, 3, 1].into_iter().enumerate() {
        recency.focused(Some(window), start + Duration::from_secs(step as u64));
    }
    assert_eq!(order(&recency), [1, 3, 2]);
    recency.forget(3);
    assert_eq!(order(&recency), [1, 2]);
    recency.focused(None, start + Duration::from_secs(9));
    assert_eq!(recency.current(), None);
    assert_eq!(order(&recency), [1, 2]);
}

#[test]
fn pointer_glance_counts_only_after_the_dwell() {
    let start = Instant::now();
    let mut recency = Recency::default();
    recency.focused(Some(1), start);
    recency.focused(Some(2), start + Duration::from_secs(1));
    recency.focused_by_pointer(2);
    recency.focused(Some(3), start + Duration::from_secs(1) + FOCUS_DWELL / 2);
    assert_eq!(order(&recency), [3, 1]);

    recency.focused_by_pointer(3);
    recency.focused(Some(4), start + Duration::from_secs(2) + FOCUS_DWELL);
    assert_eq!(order(&recency), [4, 3, 1]);
}
