#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlaneKind {
    Primary,
    Cursor,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlaneFacts {
    pub(crate) kind: PlaneKind,
    pub(crate) possible_crtcs: u32,
    pub(crate) attached_to: Option<usize>,
    pub(crate) stacking: Stacking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stacking {
    pub(crate) zpos: Option<u64>,
    pub(crate) id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Claimed {
    pub(crate) primary: usize,
    pub(crate) cursor: Option<usize>,
    pub(crate) overlay: Option<usize>,
}

pub(crate) fn pick_crtc(current: Option<usize>, possible: u32, taken: u32) -> Option<usize> {
    let free = |index: usize| index < 32 && taken & (1 << index) == 0;
    if let Some(current) = current.filter(|index| free(*index) && possible & (1 << index) != 0) {
        return Some(current);
    }
    (0..32).find(|index| possible & (1 << index) != 0 && free(*index))
}

pub(crate) fn claim_planes(
    planes: &[PlaneFacts],
    crtc: usize,
    claimed: &[bool],
) -> Option<Claimed> {
    let pick = |kind: PlaneKind, fits: &dyn Fn(&PlaneFacts) -> bool| {
        let usable = |index: &usize| {
            let plane = &planes[*index];
            plane.kind == kind
                && plane.possible_crtcs & (1 << crtc) != 0
                && !claimed.get(*index).copied().unwrap_or(false)
                && fits(plane)
        };
        let candidates = || (0..planes.len()).filter(usable);
        candidates()
            .find(|index| planes[*index].attached_to == Some(crtc))
            .or_else(|| candidates().find(|index| planes[*index].attached_to.is_none()))
            .or_else(|| candidates().next())
    };
    let primary = pick(PlaneKind::Primary, &|_| true)?;
    let under = planes[primary].stacking;
    Some(Claimed {
        primary,
        cursor: pick(PlaneKind::Cursor, &|_| true),
        overlay: pick(PlaneKind::Overlay, &|plane| plane.stacking.above(under)),
    })
}

impl Stacking {
    fn above(self, under: Stacking) -> bool {
        match (self.zpos, under.zpos) {
            (Some(zpos), Some(beneath)) => (zpos, self.id) > (beneath, under.id),
            _ => true,
        }
    }
}
