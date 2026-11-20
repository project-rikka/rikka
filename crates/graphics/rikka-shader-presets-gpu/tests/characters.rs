use rikka_renderer::Renderer;
use rikka_renderer_3d::glam::{Mat4, Vec3, Vec4};
use rikka_renderer_3d::{
    DEFAULT_LAYERS, Draw, FACE_LAYER, Figure, HAIR_LAYER, Head, MaterialDesc, Overrides,
    SEE_THROUGH_LAYER,
};

use crate::support::{Shot, create, frame, looked, opaque, quad, renderer, sun};

fn anime(class: &str) -> MaterialDesc {
    opaque("anime").preset(class).expect("an anime class")
}

fn procedural_face_shot(renderer: &mut Renderer, azimuth: f32, elevation: f32) -> Shot {
    let plane = quad(renderer, 1.0);
    let face = create(
        renderer,
        anime("face")
            .with("base_color", Vec4::ONE)
            .and_then(|desc| desc.with("shade_color", Vec3::splat(0.3)))
            .expect("face parameters"),
    );
    let mut frame = frame(Some(looked()));
    let figure = frame.figure(Figure {
        head: Some(Head {
            center: Vec3::new(0.0, 0.0, -0.6),
            forward: Vec3::Z,
            up: Vec3::Y,
            radius: 1.0,
        }),
        ..Figure::default()
    });
    let (azimuth, elevation) = (azimuth.to_radians(), elevation.to_radians());
    frame.light(sun(Vec3::new(
        azimuth.sin() * elevation.cos(),
        elevation.sin(),
        azimuth.cos() * elevation.cos(),
    )));
    frame.draw(Draw::new(plane, face).figure(figure));
    Shot::take(renderer, &frame)
}

#[test]
fn anime_face_shadow_sweeps_vertically_from_the_far_side() {
    let mut renderer = renderer();
    let columns = 30..98;
    let lit = |shot: &Shot, row: u32| -> Vec<bool> {
        columns.clone().map(|x| shot.at(x, row).x > 205.0).collect()
    };
    for elevation in [-50.0, 0.0, 50.0] {
        for azimuth in [-90.0, -40.0, 40.0, 90.0] {
            let shot = procedural_face_shot(&mut renderer, azimuth, elevation);
            let name = format!("light at {azimuth}°, {elevation}° up");
            let forehead = lit(&shot, 37);
            for row in [52, 95] {
                assert_eq!(
                    lit(&shot, row),
                    forehead,
                    "{name}: row {row} shades like the forehead"
                );
            }
            let shaded = forehead.iter().filter(|lit| !**lit).count();
            let far_edge = match azimuth > 0.0 {
                true => forehead.iter().take_while(|lit| !**lit).count(),
                false => forehead.iter().rev().take_while(|lit| !**lit).count(),
            };
            assert_eq!(
                shaded, far_edge,
                "{name}: the shade is one band on the far side"
            );
            match f32::abs(azimuth) < 60.0 {
                true => assert!(
                    shaded * 4 < forehead.len(),
                    "{name}: a 40° key keeps the face lit: {shaded}"
                ),
                false => assert!(
                    shaded * 3 > forehead.len() && shaded * 3 < forehead.len() * 2,
                    "{name}: a side key shades about half: {shaded} of {}",
                    forehead.len()
                ),
            }
        }
    }
}

fn hair_scene(renderer: &mut Renderer, receiver: MaterialDesc, figure: Overrides) -> Shot {
    let plane = quad(renderer, 1.0);
    let strip = quad(renderer, 0.5);
    let face = create(
        renderer,
        receiver
            .with("base_color", Vec4::ONE)
            .and_then(|desc| desc.with("shade_color", Vec3::splat(0.3)))
            .and_then(|desc| desc.with("hair_shadow_distance", 0.4))
            .and_then(|desc| desc.with("hair_shadow_reach", 0.5))
            .expect("receiver parameters"),
    );
    let hair = create(
        renderer,
        anime("hair")
            .with("base_color", Vec4::new(0.1, 0.1, 0.1, 1.0))
            .expect("hair parameters"),
    );
    let mut frame = frame(Some(looked()));
    let figure = frame.figure(Figure::new(figure));
    frame.light(sun(Vec3::new(0.0, 1.0, 0.6)));
    frame.draw(
        Draw::new(plane, face)
            .figure(figure)
            .layers(DEFAULT_LAYERS | FACE_LAYER),
    );
    frame.draw(
        Draw::new(strip, hair)
            .transform(Mat4::from_translation(Vec3::new(0.0, 1.0, 0.1)))
            .layers(DEFAULT_LAYERS | HAIR_LAYER),
    );
    Shot::take(renderer, &frame)
}

#[test]
fn hair_shadow_lands_only_where_it_is_turned_up() {
    let mut renderer = renderer();
    let face = hair_scene(&mut renderer, anime("face"), Overrides::new());
    let cloth = hair_scene(&mut renderer, anime("cloth"), Overrides::new());
    let below_hair = |shot: &Shot| shot.brightness(54..74, 54);
    let far_below = |shot: &Shot| shot.brightness(54..74, 96);
    assert!(
        below_hair(&face) < far_below(&face) * 0.8,
        "the face darkens just below the hair: {} vs {}",
        below_hair(&face),
        far_below(&face)
    );
    assert!(
        below_hair(&cloth) > far_below(&cloth) * 0.9,
        "cloth starts with no hair shadow: {} vs {}",
        below_hair(&cloth),
        far_below(&cloth)
    );
    let receiving = anime("cloth")
        .with("hair_shadow", 1.0)
        .expect("a receiving cloth");
    let by_value = hair_scene(&mut renderer, receiving, Overrides::new());
    let opted_in = hair_scene(
        &mut renderer,
        anime("cloth"),
        Overrides::new().with("hair_shadow", 1.0),
    );
    assert!(
        below_hair(&by_value) < far_below(&by_value) * 0.8,
        "a receiving value darkens cloth: {} vs {}",
        below_hair(&by_value),
        far_below(&by_value)
    );
    assert!(
        (below_hair(&opted_in) - below_hair(&by_value)).abs() < 1.0,
        "an override turns the receiver on like the value: {} vs {}",
        below_hair(&opted_in),
        below_hair(&by_value)
    );
}

fn eye_behind_hair(renderer: &mut Renderer, layers: u32, color: Vec3) -> Vec3 {
    let plane = quad(renderer, 0.5);
    let eye = create(
        renderer,
        opaque("mmd")
            .with("base_color", color.extend(1.0))
            .and_then(|desc| desc.with("shade_color", color))
            .and_then(|desc| desc.with("see_through.amount", 0.35))
            .and_then(|desc| desc.with("see_through.reach", 0.08))
            .expect("eye parameters"),
    );
    let hair = create(
        renderer,
        opaque("mmd")
            .with("base_color", Vec4::new(0.0, 1.0, 0.0, 1.0))
            .and_then(|desc| desc.with("shade_color", Vec3::new(0.0, 1.0, 0.0)))
            .expect("hair parameters"),
    );
    let mut frame = frame(Some(looked()));
    frame.light(sun(Vec3::new(0.0, 0.3, 1.0)));
    frame.draw(Draw::new(plane, eye).layers(layers));
    frame.draw(
        Draw::new(plane, hair)
            .transform(Mat4::from_translation(Vec3::new(0.0, 0.0, 0.03)))
            .layers(DEFAULT_LAYERS | HAIR_LAYER),
    );
    Shot::take(renderer, &frame).at(64, 64)
}

#[test]
fn eyes_show_through_bangs_only_on_the_see_through_layer() {
    let mut renderer = renderer();
    let red = Vec3::new(1.0, 0.0, 0.0);
    let skin = Vec3::new(0.95, 0.68, 0.58);
    let amber = Vec3::new(0.45, 0.2, 0.07);
    let seen = DEFAULT_LAYERS | SEE_THROUGH_LAYER;
    let cases = [
        ("a red iris shows through", seen, red, true),
        (
            "a plain face part stays hidden",
            DEFAULT_LAYERS | FACE_LAYER,
            red,
            false,
        ),
        ("a skin-toned amber iris survives", seen, amber, true),
        (
            "skin painted in an eye texture stays hidden",
            seen,
            skin,
            false,
        ),
    ];
    for (name, layers, color, visible) in cases {
        let shown = eye_behind_hair(&mut renderer, layers, color);
        match visible {
            true => assert!(shown.x > 20.0 && shown.y > 40.0, "{name}: {shown}"),
            false => assert!(shown.x < 8.0, "{name}: {shown}"),
        }
    }
}
