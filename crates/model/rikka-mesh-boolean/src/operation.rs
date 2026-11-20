#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operation {
    Intersect,
    Union,
    Difference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Solver {
    #[default]
    Exact,
    Fast,
}

impl Operation {
    pub(crate) fn keeps(self, winding: &[i32]) -> bool {
        match self {
            Operation::Intersect => winding.iter().all(|&w| w != 0),
            Operation::Union => winding.iter().any(|&w| w != 0),
            Operation::Difference => {
                winding.first().is_some_and(|&w| w != 0) && winding[1..].iter().all(|&w| w < 1)
            }
        }
    }
}
