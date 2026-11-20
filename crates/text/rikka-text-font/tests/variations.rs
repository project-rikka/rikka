use rikka_test_fonts::Writer;
use rikka_text_font::{F2Dot14, Font, FontFile, Tag};

const WGHT: Tag = Tag::new(b"wght");
const OPSZ: Tag = Tag::new(b"opsz");

#[test]
fn user_values_normalize_and_clamp() {
    let data = rikka_test_fonts::font_bytes("InterVariable.ttf");
    let font = Font::new(&data).unwrap();
    assert_eq!(
        font.location([(WGHT, 900.0)]).coords(),
        [F2Dot14::ZERO, F2Dot14::ONE]
    );
    assert_eq!(
        font.location([(WGHT, 2000.0)]),
        font.location([(WGHT, 900.0)])
    );
    assert_eq!(
        font.location([(WGHT, 100.0)]).coords(),
        [F2Dot14::ZERO, F2Dot14::MINUS_ONE]
    );
    assert_eq!(
        font.location([(OPSZ, 32.0)]).coords(),
        [F2Dot14::ONE, F2Dot14::ZERO]
    );
    assert!(font.location([(Tag::new(b"wdth"), 50.0)]).is_default());
    assert!(font.location([]).is_default());
}

#[test]
fn named_instances_resolve_to_coordinates() {
    for (file, axis_values) in [("InterVariable.ttf", 2), ("NotoSansCJK-VF.otf.ttc", 1)] {
        let data = rikka_test_fonts::font_bytes(file);
        let font = FontFile::new(&data).unwrap().face(0).unwrap();
        let fvar = font.fvar().unwrap();
        let name = font.name().unwrap();
        assert!(fvar.instance_count() > 5);
        let mut previous_weight = f32::MIN;
        for (index, instance) in fvar.instances().enumerate() {
            let values: Vec<f32> = instance.coordinates().collect();
            assert_eq!(values.len(), axis_values);
            let settings = fvar.axes().map(|axis| axis.tag).zip(values.iter().copied());
            assert_eq!(
                font.named_instance_location(index as u16),
                Some(font.location(settings))
            );
            assert!(name.get(instance.subfamily_name_id).is_some());
            if file.starts_with("Noto") {
                assert!(values[0] > previous_weight);
                previous_weight = values[0];
            }
        }
    }
}

fn fvar_table(axes: &[(&[u8; 4], f32, f32, f32)]) -> Vec<u8> {
    let mut table = Writer::new();
    table
        .u16(1)
        .u16(0)
        .u16(16)
        .u16(2)
        .u16(axes.len() as u16)
        .u16(20)
        .u16(0)
        .u16(axes.len() as u16 * 4 + 4);
    for (tag, min, default, max) in axes {
        table
            .raw(*tag)
            .fixed(*min)
            .fixed(*default)
            .fixed(*max)
            .u16(0)
            .u16(256);
    }
    table.finish()
}

fn variation_store(region: (i16, i16, i16), delta: i16) -> Vec<u8> {
    let mut store = Writer::new();
    store.u16(1).u32(12).u16(1).u32(22);
    store
        .u16(1)
        .u16(1)
        .i16(region.0)
        .i16(region.1)
        .i16(region.2);
    store.u16(1).u16(1).u16(1).u16(0).i16(delta);
    store.finish()
}

#[test]
fn avar_segment_maps_remap_axis() {
    let mut avar = Writer::new();
    avar.u16(1).u16(0).u16(0).u16(1).u16(4);
    for (from, to) in [(-1.0, -1.0), (0.0, 0.0), (0.5, 0.8), (1.0, 1.0)] {
        avar.i16(F2Dot14::from_f32(from).0)
            .i16(F2Dot14::from_f32(to).0);
    }
    let data = crate::support::sfnt(
        1,
        &[
            (b"fvar", fvar_table(&[(b"wght", 100.0, 400.0, 900.0)])),
            (b"avar", avar.finish()),
        ],
    );
    let font = Font::new(&data).unwrap();
    assert_eq!(font.location([(WGHT, 525.0)]).coord(0), F2Dot14(6554));
    assert_eq!(
        font.location([(WGHT, 650.0)]).coord(0),
        F2Dot14::from_f32(0.8)
    );
    assert_eq!(
        font.location([(WGHT, 775.0)]).coord(0),
        F2Dot14::from_f32(0.9)
    );
    assert_eq!(
        font.location([(WGHT, 250.0)]).coord(0),
        F2Dot14::from_f32(-0.5)
    );
    assert_eq!(font.location([(WGHT, 900.0)]).coord(0), F2Dot14::ONE);
}

#[test]
fn avar2_shifts_normalized_coordinates() {
    let mut avar = Writer::new();
    avar.u16(2)
        .u16(0)
        .u16(0)
        .u16(2)
        .u16(0)
        .u16(0)
        .u32(0)
        .u32(20);
    avar.raw(&variation_store((0, 16384, 16384), 4096));
    let data = crate::support::sfnt(
        1,
        &[
            (
                b"fvar",
                fvar_table(&[(b"wght", 0.0, 0.0, 1.0), (b"wdth", 0.0, 0.0, 1.0)]),
            ),
            (b"avar", avar.finish()),
        ],
    );
    let font = Font::new(&data).unwrap();
    let wdth = Tag::new(b"wdth");
    assert_eq!(
        font.location([(WGHT, 0.5)]).coords(),
        [F2Dot14(8192 + 2048), F2Dot14(0)]
    );
    assert_eq!(
        font.location([(WGHT, 1.0)]).coords(),
        [F2Dot14::ONE, F2Dot14(0)]
    );
    assert_eq!(
        font.location([(wdth, 1.0)]).coords(),
        [F2Dot14(0), F2Dot14::ONE]
    );
    assert_eq!(
        font.location([(WGHT, 0.25), (wdth, 0.5)]).coords(),
        [F2Dot14(4096 + 1024), F2Dot14(8192)]
    );
}
