use std::sync::Arc;

use animate_cast::{Cast, Prop, Subject};
use animate_prop::PropKind;
use rikka_model::Model;

fn prop(kind: PropKind, name: &str) -> Arc<Prop> {
    Arc::new(Prop::load(kind, name.to_owned(), Model::default(), None, None).prop)
}

#[test]
fn a_new_stage_replaces_the_old_one() {
    let mut cast = Cast::default();
    let first = prop(PropKind::Stage, "Hall");
    cast.set_stage(Some(Arc::clone(&first)));
    let second = prop(PropKind::Stage, "Beach");
    cast.set_stage(Some(Arc::clone(&second)));
    let stage = cast.stage().expect("a stage");
    assert_eq!(stage.prop.id, second.id);
    assert_eq!(cast.props().count(), 1);
    cast.set_stage(None);
    assert!(cast.stage().is_none());
}

#[test]
fn props_follow_figures_and_dropping_an_accessory_picks_a_neighbour() {
    let mut cast = Cast::default();
    let ribbon = prop(PropKind::Accessory, "Ribbon");
    let sword = prop(PropKind::Accessory, "Sword");
    cast.place_accessory(Arc::clone(&ribbon));
    cast.place_accessory(Arc::clone(&sword));
    cast.set_stage(Some(prop(PropKind::Stage, "Hall")));
    assert_eq!(
        cast.subjects(),
        [
            Subject::Stage,
            Subject::Accessory(ribbon.id),
            Subject::Accessory(sword.id),
            Subject::Camera,
            Subject::Look,
            Subject::Wind,
        ]
    );
    assert_eq!(
        cast.drop_accessory(ribbon.id),
        Some(Subject::Accessory(sword.id))
    );
    assert_eq!(cast.drop_accessory(sword.id), Some(Subject::Stage));
}
