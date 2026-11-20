use rikka_test_fonts::Writer;

pub(crate) const UNITS_PER_EM: u16 = 1000;
pub(crate) const GLYPH_COUNT: u16 = 40;

const LONG_LOCA_OFFSETS: u16 = 1;
const HEAD_INDEX_TO_LOC_FORMAT: usize = 50;
const HHEA_SIZE: usize = 36;
const HHEA_METRIC_COUNT: usize = 34;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Square {
    pub(crate) glyph: u16,
    pub(crate) x_min: i16,
    pub(crate) y_min: i16,
    pub(crate) x_max: i16,
    pub(crate) y_max: i16,
}

fn simple_glyph(square: Square) -> Vec<u8> {
    let mut w = Writer::new();
    w.i16(1)
        .i16(square.x_min)
        .i16(square.y_min)
        .i16(square.x_max)
        .i16(square.y_max)
        .u16(3)
        .u16(0)
        .raw(&[1, 1, 1, 1]);
    let xs = [square.x_min, square.x_min, square.x_max, square.x_max];
    let ys = [square.y_min, square.y_max, square.y_max, square.y_min];
    let mut previous = 0;
    for x in xs {
        w.i16(x - previous);
        previous = x;
    }
    previous = 0;
    for y in ys {
        w.i16(y - previous);
        previous = y;
    }
    w.align(4).finish()
}

pub(crate) fn font(squares: &[Square], extra: Vec<(&[u8; 4], Vec<u8>)>) -> Vec<u8> {
    let mut glyf = Vec::new();
    let mut loca = Writer::new();
    let mut hmtx = Writer::new();
    for glyph in 0..GLYPH_COUNT {
        loca.u32(glyf.len() as u32);
        let square = squares.iter().find(|square| square.glyph == glyph);
        hmtx.u16(UNITS_PER_EM)
            .i16(square.map_or(0, |square| square.x_min));
        if let Some(square) = square {
            glyf.extend_from_slice(&simple_glyph(*square));
        }
    }
    loca.u32(glyf.len() as u32);
    let mut head = rikka_test_fonts::head(UNITS_PER_EM);
    head[HEAD_INDEX_TO_LOC_FORMAT..HEAD_INDEX_TO_LOC_FORMAT + 2]
        .copy_from_slice(&LONG_LOCA_OFFSETS.to_be_bytes());
    let mut hhea = vec![0u8; HHEA_SIZE];
    hhea[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    hhea[HHEA_METRIC_COUNT..HHEA_METRIC_COUNT + 2].copy_from_slice(&GLYPH_COUNT.to_be_bytes());
    let mut tables: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"head", head),
        (b"hhea", hhea),
        (b"maxp", rikka_test_fonts::maxp(GLYPH_COUNT)),
        (b"hmtx", hmtx.finish()),
        (b"loca", loca.finish()),
        (b"glyf", glyf),
    ];
    tables.extend(extra);
    rikka_test_fonts::sfnt(&tables)
}
