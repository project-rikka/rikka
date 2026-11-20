use rikka_weather::{Place, Spot};

fn place(latitude: f64, longitude: f64) -> Place {
    Place {
        name: String::new(),
        region: None,
        country: None,
        country_code: None,
        latitude,
        longitude,
    }
}

#[test]
fn distance_uses_great_circle() {
    let tokyo = place(35.6895, 139.6917);

    let osaka_metres = tokyo.metres_to(34.6937, 135.5023);

    assert!(
        (390_000.0..410_000.0).contains(&osaka_metres),
        "{osaka_metres}"
    );
    assert!(tokyo.metres_to(35.6895, 139.6917) < 1e-6);
}

#[test]
fn only_valid_place_is_used() {
    let tokyo = place(35.69, 139.69);

    assert_eq!(
        Spot::from_setting("place", Some(tokyo.clone())),
        Spot::Fixed(tokyo.clone())
    );
    assert_eq!(Spot::from_setting("here", Some(tokyo)), Spot::Here);
    assert_eq!(Spot::from_setting("place", None), Spot::Here);
    assert_eq!(
        Spot::from_setting("place", Some(place(91.0, 0.0))),
        Spot::Here
    );
    assert_eq!(
        Spot::from_setting("place", Some(place(0.0, 200.0))),
        Spot::Here
    );
}
