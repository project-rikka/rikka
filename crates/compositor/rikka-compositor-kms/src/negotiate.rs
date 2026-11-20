use rikka_compositor_types::{Format, FormatSet, Fourcc, Modifier};

use crate::hdr::Signal;

const SDR: [Fourcc; 2] = [Fourcc::XRGB8888, Fourcc::ARGB8888];
const HDR: [Fourcc; 4] = [
    Fourcc::XRGB2101010,
    Fourcc::XBGR2101010,
    Fourcc::ARGB2101010,
    Fourcc::ABGR2101010,
];

pub(crate) fn preferred(signal: &Signal) -> &'static [Fourcc] {
    match signal {
        Signal::Sdr => &SDR,
        Signal::Pq(_) => &HDR,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub(crate) fourcc: Fourcc,
    pub(crate) modifiers: Vec<Modifier>,
    pub(crate) linear: bool,
}

impl Candidate {
    pub(crate) fn is_implicit(&self) -> bool {
        self.modifiers.iter().all(|modifier| modifier.is_implicit())
    }

    pub(crate) fn accepts(&self, renderable: &FormatSet, allocated: Modifier) -> bool {
        renderable.contains(Format::new(self.fourcc, allocated))
            || (self.is_implicit()
                && allocated.is_implicit()
                && renderable.contains(Format::new(self.fourcc, Modifier::INVALID)))
    }
}

pub(crate) fn candidates(
    plane: &FormatSet,
    renderable: &FormatSet,
    explicit_modifiers: bool,
    preferred: &[Fourcc],
) -> Vec<Candidate> {
    let mut explicit = Vec::new();
    let mut implicit = Vec::new();
    for &fourcc in preferred {
        if !plane.supports(fourcc) || !renderable.supports(fourcc) {
            continue;
        }
        let shared: Vec<Modifier> = plane
            .modifiers(fourcc)
            .filter(|modifier| !modifier.is_implicit())
            .filter(|modifier| renderable.contains(Format::new(fourcc, *modifier)))
            .collect();
        if explicit_modifiers && !shared.is_empty() {
            explicit.push(Candidate {
                fourcc,
                modifiers: shared,
                linear: false,
            });
        }
        implicit.push(Candidate {
            fourcc,
            modifiers: vec![Modifier::INVALID],
            linear: false,
        });
    }
    explicit.extend(implicit);
    explicit
}

pub(crate) fn linear_candidates(
    plane: &FormatSet,
    renderable: &FormatSet,
    explicit_modifiers: bool,
    preferred: &[Fourcc],
) -> Vec<Candidate> {
    linear_fourccs(plane, preferred)
        .filter(|fourcc| renderable.contains(Format::new(*fourcc, Modifier::LINEAR)))
        .map(|fourcc| {
            let explicit =
                explicit_modifiers && plane.contains(Format::new(fourcc, Modifier::LINEAR));
            Candidate {
                fourcc,
                modifiers: vec![match explicit {
                    true => Modifier::LINEAR,
                    false => Modifier::INVALID,
                }],
                linear: true,
            }
        })
        .collect()
}

pub(crate) fn linear_fourccs<'a>(
    plane: &'a FormatSet,
    preferred: &'a [Fourcc],
) -> impl Iterator<Item = Fourcc> + 'a {
    preferred.iter().copied().filter(|fourcc| {
        plane.contains(Format::new(*fourcc, Modifier::LINEAR))
            || plane.contains(Format::new(*fourcc, Modifier::INVALID))
    })
}
