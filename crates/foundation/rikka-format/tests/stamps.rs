use rikka_format::format_timestamp;

#[test]
fn timestamp_formats_local_minute() {
    assert_eq!(format_timestamp(0), "1970/01/01 00:00");
    assert_eq!(format_timestamp(1_700_000_059), "2023/11/14 22:14");
    assert_eq!(format_timestamp(i64::MAX), "");
}
