use paint_assistant::{Framing, MAX_PATCHES, MAX_SIDE, PATCH, Rect};

fn rect(left: u32, top: u32, width: u32, height: u32) -> Rect {
    Rect {
        left,
        top,
        width,
        height,
    }
}

fn patches(framing: &Framing) -> u32 {
    let (width, height) = framing.sheet();
    width.div_ceil(PATCH) * height.div_ceil(PATCH)
}

#[test]
fn framing_respects_provider_limits() {
    for (width, height) in [(4000, 4000), (8000, 100), (100, 8000), (3000, 2000)] {
        for panels in [1, 2] {
            let framing = Framing::fit(rect(0, 0, width, height), MAX_SIDE, panels);
            let (across, down) = framing.sheet();
            assert!(across <= MAX_SIDE && down <= MAX_SIDE, "{framing:?}");
            assert!(patches(&framing) <= MAX_PATCHES, "{framing:?}");
        }
    }
    let square = Framing::fit(rect(0, 0, 4000, 4000), MAX_SIDE, 1);
    assert_eq!(square.width, 1092);
}

#[test]
fn framing_maps_document_to_image_pixels() {
    let framing = Framing::fit(rect(100, 50, 400, 200), 800, 1);
    assert_eq!(framing.image_x(100.0), 0.0);
    assert_eq!(framing.image_x(300.0), 400.0);
    assert_eq!(framing.image_y(150.0), 200.0);
    let mapping = framing.mapping();
    assert!(
        mapping.contains("x 100–500") && mapping.contains("y 50–250"),
        "{mapping}"
    );
    assert!(mapping.contains("scale 2"), "{mapping}");
}
