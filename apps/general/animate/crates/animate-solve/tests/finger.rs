use animate_body::{Body, Dof, LEFT_FINGERS, Side, muscles};
use animate_solve::Finger;

use crate::support::{BODY, changed, rest};

#[test]
fn finger_flex_curls_and_spreads_base() {
    let body: &Body = &BODY;
    let pose = rest();
    for finger in Finger::all(Side::Left) {
        assert!(finger.fits(body));
        let curled = finger.flex(body, &pose, 0.5, 0.0, true);
        let bends = changed(&pose, &curled);
        assert_eq!(bends.len(), 3, "{finger:?}");
        assert!(
            bends
                .iter()
                .all(|index| LEFT_FINGERS.contains(index) && muscles()[*index].dof == Dof::Bend)
        );
        let spread = finger.flex(body, &pose, 0.0, 0.5, true);
        let sides = changed(&pose, &spread);
        assert_eq!(sides.len(), 1, "{finger:?}");
        assert_eq!(muscles()[sides[0]].dof, Dof::Side);
        let fist = finger.flex(body, &pose, 10.0, 0.0, true);
        assert!(fist.muscles.iter().all(|value| value.abs() <= 1.0 + 1e-6));
    }
}
