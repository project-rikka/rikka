use rikka_renderer::{Blur, Joined, PostEffect, Spread};
use rikka_types::{CornerRadii, Lens, Point, Rect, RoundedRect};

#[test]
fn wide_pane_lens_spread_covers_inward_bulge_refraction() {
    let pane = RoundedRect::new(
        Rect::new(0.0, 0.0, 1200.0, 80.0),
        CornerRadii::uniform(20.0),
    );
    let blur = Blur::new(3.2);
    let lens = Lens::new();
    let spread = Spread::new(pane, &blur, Some(lens), None);

    let changed = Rect::new(1078.0, 250.0, 10.0, 10.0);
    let mut stale = Vec::new();
    spread.stale_regions(changed, &mut stale);

    let rim_pixel = Point::new(1110.5, 79.5);
    assert!(
        stale.iter().any(|rect| rect.contains(rim_pixel)),
        "inward bulge + outward bevel rim pixel {rim_pixel:?} must be covered by {stale:?}"
    );
}

#[test]
fn joined_glass_spread_covers_aligned_bevel_and_bulge() {
    let host = RoundedRect::new(Rect::new(0.0, 0.0, 1200.0, 64.0), CornerRadii::ZERO);
    let limb = RoundedRect::new(
        Rect::new(900.0, 64.0, 260.0, 180.0),
        CornerRadii::new(0.0, 0.0, 16.0, 16.0),
    );
    let joined = Joined::new(host, limb).expect("limb hangs square off host");
    let blur = Blur::new(4.0);
    let lens = Lens::new();
    let spread = Spread::new(joined.limb(), &blur, Some(lens), Some(joined));

    let profile = lens.profile_within(joined.host().rect, joined.limb().rect);
    assert!(
        spread.reads() >= profile.bevel_displacement + blur.spread(joined.limb().rect.size),
        "joined spread reads ({}) must include both blur and refraction",
        spread.reads()
    );

    let changed = Rect::new(
        joined.limb().rect.min_x() - profile.bevel_displacement - profile.bulge_displacement * 0.5,
        120.0,
        12.0,
        12.0,
    );
    let mut stale = Vec::new();
    spread.stale_regions(changed, &mut stale);
    let host_facing_edge = Point::new(joined.limb().rect.min_x() + 2.0, 126.0);
    assert!(
        stale.iter().any(|rect| rect.contains(host_facing_edge)),
        "host-facing limb edge {host_facing_edge:?} must be covered when bevel and bulge align: {stale:?}"
    );
}
