use layout_bench::{Engine, Extent, Placed, Rikka, SCENARIOS, Scenario, Spec, Taffy, Yoga};

const TOLERANCE: f32 = 1.0;

fn laid<E: Engine>(spec: &Spec) -> Vec<Placed> {
    let mut engine = E::build(spec);
    engine.compute();
    engine.placed()
}

fn close(a: &Placed, b: &Placed) -> bool {
    (a.x - b.x).abs() <= TOLERANCE
        && (a.y - b.y).abs() <= TOLERANCE
        && (a.width - b.width).abs() <= TOLERANCE
        && (a.height - b.height).abs() <= TOLERANCE
}

fn assert_agree(scenario: &Scenario, label: &str, spec: &Spec, reference: &[Placed]) {
    let checks = [
        (Rikka::NAME, laid::<Rikka>(spec)),
        (Yoga::NAME, laid::<Yoga>(spec)),
    ];
    for (engine, placed) in checks {
        assert_eq!(placed.len(), reference.len(), "{engine} placed every node");
        if let Some((index, (got, want))) = placed
            .iter()
            .zip(reference)
            .enumerate()
            .find(|(_, (got, want))| !close(got, want))
        {
            panic!(
                "{} {label}: {engine} put node {index} at {got:?}, the reference at {want:?}",
                scenario.name
            );
        }
    }
}

#[test]
fn engines_agree_on_every_scenario() {
    for scenario in SCENARIOS {
        let spec = scenario.spec();
        let reference = laid::<Taffy>(&spec);
        assert_agree(scenario, "initial", &spec, &reference);
    }
}

#[test]
fn incremental_edits_match_fresh_layout() {
    for scenario in SCENARIOS {
        let spec = scenario.spec();
        let leaves = spec.leaves();
        let leaf = leaves[leaves.len() / 2];
        let grown = Extent::new(14.0, 10.0);
        let mut edited = spec.clone();
        edited.visit_mut(&mut 0, &mut |index, node| {
            if index == leaf {
                node.content = Some(grown);
            }
        });
        let width = spec.root_extent().width + 1.0;
        edited.style.width = Some(width);
        let fresh = laid::<Taffy>(&edited);
        for (engine, placed) in [
            (Rikka::NAME, edit::<Rikka>(&spec, leaf, grown, width)),
            (Taffy::NAME, edit::<Taffy>(&spec, leaf, grown, width)),
            (Yoga::NAME, edit::<Yoga>(&spec, leaf, grown, width)),
        ] {
            assert_eq!(placed.len(), fresh.len(), "{engine} placed every node");
            assert!(
                placed
                    .iter()
                    .zip(&fresh)
                    .all(|(got, want)| close(got, want)),
                "{}: {engine} kept stale positions after an edit",
                scenario.name
            );
        }
    }
}

fn edit<E: Engine>(spec: &Spec, leaf: usize, content: Extent, width: f32) -> Vec<Placed> {
    let mut engine = E::build(spec);
    engine.compute();
    engine.resize_leaf(leaf, content);
    engine.resize(width);
    engine.compute();
    engine.placed()
}
