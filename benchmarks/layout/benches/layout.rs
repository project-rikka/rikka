use divan::Bencher;
use divan::counter::ItemsCount;
use layout_bench::{Engine, Extent, Rikka, Scenario, Taffy, Yoga};

fn main() {
    divan::main();
}

fn build_and_compute<E: Engine>(bencher: Bencher, scenario: &Scenario) {
    let spec = scenario.spec();
    bencher
        .counter(ItemsCount::new(spec.count()))
        .bench_local(|| {
            let mut engine = E::build(&spec);
            engine.compute();
            engine
        });
}

fn relayout<E: Engine>(bencher: Bencher, scenario: &Scenario) {
    let spec = scenario.spec();
    let width = spec.root_extent().width;
    let mut engine = E::build(&spec);
    engine.compute();
    let mut wider = false;
    bencher
        .counter(ItemsCount::new(spec.count()))
        .bench_local(|| {
            wider = !wider;
            engine.resize(if wider { width + 1.0 } else { width });
            engine.compute();
        });
}

fn incremental<E: Engine>(bencher: Bencher, scenario: &Scenario) {
    let spec = scenario.spec();
    let leaves = spec.leaves();
    let leaf = leaves[leaves.len() / 2];
    let mut engine = E::build(&spec);
    engine.compute();
    let mut grown = false;
    bencher.bench_local(|| {
        grown = !grown;
        let width = if grown { 14.0 } else { 10.0 };
        engine.resize_leaf(leaf, Extent::new(width, 10.0));
        engine.compute();
    });
}

macro_rules! scenarios {
    ($($scenario:ident => $constant:ident),* $(,)?) => {
        $(
            mod $scenario {
                use super::*;

                #[divan::bench(types = [Rikka, Taffy, Yoga])]
                fn build<E: Engine>(bencher: Bencher) {
                    build_and_compute::<E>(bencher, &layout_bench::$constant);
                }

                #[divan::bench(types = [Rikka, Taffy, Yoga])]
                fn resize<E: Engine>(bencher: Bencher) {
                    relayout::<E>(bencher, &layout_bench::$constant);
                }

                #[divan::bench(types = [Rikka, Taffy, Yoga])]
                fn edit_leaf<E: Engine>(bencher: Bencher) {
                    incremental::<E>(bencher, &layout_bench::$constant);
                }
            }
        )*
    };
}

scenarios! {
    stack_with_flex => STACK_WITH_FLEX,
    align_stretch => ALIGN_STRETCH,
    nested_flex => NESTED_FLEX,
    huge_nested => HUGE_NESTED,
    list => LIST,
    tiles => TILES,
    deep => DEEP,
}
