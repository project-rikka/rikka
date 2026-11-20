use std::time::Duration;

use cut_project::{Rate, Timecode};

fn formatted(frame: i64, rate: Rate) -> String {
    Timecode::from_frame(frame, rate).to_string()
}

#[test]
fn drop_frame_timecode_skips_labels() {
    assert_eq!(formatted(25 * 3661 + 3, Rate::PAL), "01:01:01:03");
    assert_eq!(formatted(-30, Rate::VIDEO), "-00:00:01:00");
    assert_eq!(formatted(1799, Rate::NTSC_VIDEO), "00:00:59;29");
    assert_eq!(formatted(1800, Rate::NTSC_VIDEO), "00:01:00;02");
    assert_eq!(formatted(17_981, Rate::NTSC_VIDEO), "00:09:59;29");
    assert_eq!(formatted(17_982, Rate::NTSC_VIDEO), "00:10:00;00");
    assert_eq!(formatted(107_892, Rate::NTSC_VIDEO), "01:00:00;00");
    assert_eq!(formatted(3600, Rate::NTSC_HIGH), "00:01:00;04");
    assert_eq!(formatted(35_964, Rate::NTSC_HIGH), "00:10:00;00");
    assert_eq!(formatted(24, Rate::NTSC_FILM), "00:00:01:00");
}

#[test]
fn timecode_parse_round_trips() {
    for rate in [Rate::PAL, Rate::NTSC_VIDEO, Rate::NTSC_HIGH] {
        for frame in [0, 7, 1799, 1800, 17_982, 107_893] {
            let label = formatted(frame, rate);
            assert_eq!(
                Timecode::parse(&label, rate),
                Some(frame),
                "{label} at {rate}"
            );
        }
    }
    assert_eq!(Timecode::parse("1:00", Rate::PAL), Some(25));
    assert_eq!(Timecode::parse("00:00:01.05", Rate::PAL), Some(30));
    assert_eq!(Timecode::parse("abc", Rate::PAL), None);
    assert_eq!(Timecode::parse("1:2:3:4:5", Rate::PAL), None);
}

#[test]
fn measured_rates_snap_to_broadcast_rates() {
    assert_eq!(Rate::measured(23.976_023), Some(Rate::NTSC_FILM));
    assert_eq!(Rate::measured(29.97), Some(Rate::NTSC_VIDEO));
    assert_eq!(Rate::measured(59.940_06), Some(Rate::NTSC_HIGH));
    assert_eq!(Rate::measured(25.0), Some(Rate::PAL));
    assert_eq!(Rate::measured(30.000_4), Some(Rate::VIDEO));
    assert_eq!(Rate::measured(12.5), Rate::new(25, 2).ok());
    assert_eq!(Rate::measured(0.0), None);
    assert_eq!(Rate::measured(f64::NAN), None);
    assert!(Rate::new(0, 1).is_err());
}

#[test]
fn fractional_rates_map_frames_exactly() {
    let rate = Rate::NTSC_VIDEO;
    assert_eq!(rate.start_of(30_000), Duration::from_secs(1001));
    assert_eq!(rate.frames_in(Duration::from_secs(1001)), 30_000);
    assert_eq!(rate.frames_in(rate.start_of(17)), 17);
}
