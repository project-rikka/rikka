use animate_hold::{HoldId, HoldLane, Holds};
use animate_track::Span;

use crate::keys;
use crate::sheet::Sheet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HoldSpan {
    pub id: HoldId,
    pub span: Span,
    pub ease: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoldRow {
    pub lane: HoldLane,
    pub spans: Vec<HoldSpan>,
}

impl Sheet {
    pub(crate) fn rehold(&mut self, holds: &Holds) {
        self.holding = holds
            .lanes()
            .into_iter()
            .map(|lane| HoldRow {
                lane: lane.lane,
                spans: lane
                    .holds
                    .iter()
                    .filter_map(|id| {
                        let hold = holds.get(*id)?;
                        Some(HoldSpan {
                            id: *id,
                            span: Span {
                                start: hold.from,
                                end: hold.until,
                            },
                            ease: hold.ease,
                        })
                    })
                    .collect(),
            })
            .collect();
        self.remark(keys::sorted(
            holds
                .iter()
                .flat_map(|hold| std::iter::once(hold.from).chain(hold.until)),
        ));
    }

    #[must_use]
    pub fn holding(&self) -> &[HoldRow] {
        &self.holding
    }
}
