#[test]
fn slices_keep_order_across_the_wrap() {
    let (mut producer, mut consumer) = rikka_ring::channel::<u32>(8);
    let mut next = 0;
    let mut expected = 0;
    let mut out = [0; 5];
    for _ in 0..20 {
        let chunk: Vec<u32> = (next..next + 5).collect();
        next += producer.push_slice(&chunk) as u32;
        let popped = consumer.pop_slice(&mut out);
        for value in &out[..popped] {
            assert_eq!(*value, expected);
            expected += 1;
        }
    }
    assert!(expected > 50);
    assert_eq!(producer.position(), u64::from(next));
}

#[test]
fn reads_stop_at_a_bound_and_skip_forward() {
    let (mut producer, mut consumer) = rikka_ring::channel::<u8>(16);
    assert_eq!(producer.push_slice(&[1, 2, 3, 4, 5, 6]), 6);
    let mut out = [0; 6];

    assert_eq!(consumer.pop_slice_before(&mut out, 2), 2);
    assert_eq!(out[..2], [1, 2]);

    consumer.skip_to(4);
    assert_eq!(consumer.position(), 4);
    consumer.skip_to(1);
    assert_eq!(consumer.position(), 4);
    consumer.skip_to(100);
    assert_eq!(consumer.position(), 6);

    assert_eq!(producer.push_slice(&[7]), 1);
    assert_eq!(consumer.pop_slice(&mut out), 1);
    assert_eq!(out[0], 7);
    assert!(consumer.is_empty());
}

#[test]
fn full_ring_refuses_more() {
    let (mut producer, consumer) = rikka_ring::channel::<u8>(4);
    assert_eq!(producer.push_slice(&[0; 9]), 4);
    assert_eq!(producer.free(), 0);
    assert_eq!(producer.push(1), Err(1));
    assert_eq!(consumer.len(), 4);
}
