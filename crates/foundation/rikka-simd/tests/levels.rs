use rikka_simd::prelude::*;

#[inline(always)]
fn isa_of<S: Simd>(simd: S) -> Isa {
    assert_eq!(simd.level().isa(), S::ISA);
    S::ISA
}

#[test]
fn dispatch_runs_each_level() {
    let levels: Vec<Level> = Level::supported().collect();
    assert_eq!(levels.first(), Some(&Level::baseline()));
    assert_eq!(levels.last(), Some(&Level::detect()));
    for level in levels {
        assert_eq!(
            rikka_simd::dispatch!(level, simd => isa_of(simd)),
            level.isa()
        );
        assert_eq!(Level::new(level.isa()), Some(level));
    }
}

#[test]
fn levels_capped_to_cpu() {
    let best = Level::detect();
    for isa in Isa::ALL {
        assert_eq!(Level::new(isa).is_some(), isa <= best.isa());
        assert_eq!(isa.is_supported(), isa <= best.isa());
        assert_eq!(best.with_max_isa(isa).isa(), isa.min(best.isa()));
    }
}
