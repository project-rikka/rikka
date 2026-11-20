#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Plan {
    pub(crate) substeps: u32,
    pub(crate) dt: f32,
}

impl Plan {
    pub(crate) fn new(span: f64, fastest: f32, reach: f32, range: (u32, u32)) -> Plan {
        if span <= 0.0 {
            return Plan {
                substeps: 0,
                dt: 0.0,
            };
        }
        let needed = (span * f64::from(fastest.max(0.0)) / f64::from(reach)).ceil();
        let substeps =
            (needed.min(f64::from(u32::MAX)) as u32).clamp(range.0.max(1), range.1.max(1));
        Plan {
            substeps,
            dt: (span / f64::from(substeps)) as f32,
        }
    }
}
