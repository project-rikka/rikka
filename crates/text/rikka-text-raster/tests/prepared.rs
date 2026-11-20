use crate::support::glyph;
use rikka_test_fonts::font_bytes;
use rikka_text_font::{Font, Location, Synthesis};
use rikka_text_raster::OutlineCache;
use rikka_text_raster::{GlyphImage, GlyphRequest, PreparedOutline, Rasterizer};

const SIZES: [f32; 8] = [9.5, 12.0, 16.0, 23.7, 48.0, 61.0, 96.0, 12.0];

fn syntheses() -> [Synthesis; 3] {
    [
        Synthesis::NONE,
        Synthesis {
            bold: true,
            ..Synthesis::NONE
        },
        Synthesis {
            bold: true,
            oblique: true,
        },
    ]
}

#[test]
fn prepared_outlines_match_direct_render() {
    let mut fresh = Rasterizer::new();
    let mut cached = Rasterizer::new();
    let (mut expected, mut actual) = (GlyphImage::new(), GlyphImage::new());
    for (file, characters) in [
        ("InterVariable.ttf", ['a', 'g', 'W', '@']),
        ("NotoSansCJK-VF.otf.ttc", ['永', '語', 'R', '&']),
    ] {
        let data = font_bytes(file);
        let font = Font::new(&data).unwrap();
        for character in characters {
            let id = glyph(&font, character);
            let mut prepared = PreparedOutline::new();
            assert_eq!(
                cached.prepare_outline(&font, id, &Location::default(), &mut prepared),
                Ok(())
            );
            for ppem in SIZES {
                for offset in [0.0, 0.25, 0.5, 0.75] {
                    for synthesis in syntheses() {
                        let request = GlyphRequest::new(id, ppem)
                            .subpixel_x(offset)
                            .synthesis(synthesis);
                        assert_eq!(fresh.render_outline(&font, &request, &mut expected), Ok(()));
                        assert_eq!(
                            cached.render_prepared(&mut prepared, &request, &mut actual),
                            Ok(())
                        );
                        assert_eq!(
                            actual, expected,
                            "{file} {character} at {ppem} px, offset {offset}, {synthesis:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn cache_respects_budget_and_recency() {
    let budget = 64 << 10;
    let mut cache = OutlineCache::with_budget(budget);
    let favourite = 0u16;
    let mut slot = 0;
    for glyph in 1..=u16::MAX {
        assert!(cache.outline(favourite, |_| Ok(())).is_ok());
        let before = cache.bytes();
        assert!(cache.outline(glyph, |_| Ok(())).is_ok());
        slot = slot.max(cache.bytes().saturating_sub(before));
    }
    assert!(cache.bytes() <= budget + slot);
    assert!(cache.len() < usize::from(u16::MAX));
    let mut prepared = false;
    let cached = cache.outline(favourite, |_| {
        prepared = true;
        Ok(())
    });
    assert!(cached.is_ok());
    assert!(!prepared);
}
