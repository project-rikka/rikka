use rikka_format::{format_bytes, group_digits};

#[test]
fn bytes_switch_units_at_boundaries() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1023), "1023 B");
    assert_eq!(format_bytes(1 << 10), "1.0 KiB");
    assert_eq!(format_bytes(1536), "1.5 KiB");
    assert_eq!(format_bytes(1 << 20), "1.0 MiB");
    assert_eq!(format_bytes(1 << 60), "1.0 EiB");
    assert_eq!(format_bytes(u64::MAX), "16.0 EiB");
}

#[test]
fn bytes_rounding_carries_to_next_unit() {
    assert_eq!(format_bytes(1_048_525), "1.0 MiB");
    assert_eq!(format_bytes(1_048_524), "1023.9 KiB");
    assert_eq!(format_bytes((1 << 20) - 1), "1.0 MiB");
    for bit in 0..64 {
        for offset in [0_u64, 1, 511, 512, 513, 1023] {
            let count = (1_u64 << bit).saturating_add(offset);
            let shown = format_bytes(count);
            assert!(!shown.starts_with("1024."), "{count} read as {shown}");
        }
    }
}

#[test]
fn digits_group_in_threes_before_the_point() {
    let cases = [
        ("0", "0"),
        ("999", "999"),
        ("1000", "1,000"),
        ("1234567", "1,234,567"),
        ("-1234", "-1,234"),
        ("-123", "-123"),
        ("1234.5678", "1,234.5678"),
    ];
    for (plain, grouped) in cases {
        assert_eq!(group_digits(plain), grouped);
    }
}
