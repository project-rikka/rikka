use rikka_physics_collision::Filter;

#[test]
fn groups_override_category_masks() {
    let a = Filter::default().group(-3);
    let b = Filter::default().group(-3);
    assert!(!a.should_collide(&b));
    let c = Filter::default().group(2).mask(0);
    assert!(c.should_collide(&Filter::default().group(2)));
    let ground = Filter::default().category(1 << 1).mask(1);
    let body = Filter::default().category(1).mask(1 << 1);
    assert!(ground.should_collide(&body));
    assert!(!body.should_collide(&Filter::default().category(1 << 2)));
}
