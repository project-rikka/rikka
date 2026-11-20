use crate::window::XWindowId;

const INPUT_HINT: u32 = 1;
const STATE_HINT: u32 = 1 << 1;
const WINDOW_GROUP_HINT: u32 = 1 << 6;
const URGENCY_HINT: u32 = 1 << 8;

const USER_POSITION: u32 = 1;
const PROGRAM_POSITION: u32 = 1 << 2;
const MIN_SIZE: u32 = 1 << 4;
const MAX_SIZE: u32 = 1 << 5;
const RESIZE_INCREMENT: u32 = 1 << 6;
const ASPECT: u32 = 1 << 7;
const BASE_SIZE: u32 = 1 << 8;
const WIN_GRAVITY: u32 = 1 << 9;

const MOTIF_DECORATIONS: u32 = 1 << 1;
const DECORATE_ALL: u32 = 1;
const DECORATE_BORDER: u32 = 1 << 1;
const DECORATE_TITLE: u32 = 1 << 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialState {
    Normal,
    Iconic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WmHints {
    pub input: Option<bool>,
    pub initial_state: Option<InitialState>,
    pub urgent: bool,
    pub group: Option<XWindowId>,
}

impl WmHints {
    pub(crate) fn parse(words: &[u32]) -> Self {
        let word = |index: usize| words.get(index).copied();
        let Some(flags) = word(0) else {
            return Self::default();
        };
        let when = |bit: u32, index: usize| (flags & bit != 0).then(|| word(index)).flatten();
        Self {
            input: when(INPUT_HINT, 1).map(|input| input != 0),
            initial_state: when(STATE_HINT, 2).and_then(|state| match state {
                1 => Some(InitialState::Normal),
                3 => Some(InitialState::Iconic),
                _ => None,
            }),
            urgent: flags & URGENCY_HINT != 0,
            group: when(WINDOW_GROUP_HINT, 8)
                .filter(|group| *group != 0)
                .map(XWindowId::new),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ratio {
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AspectRange {
    pub min: Ratio,
    pub max: Ratio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SizeHints {
    pub position: Option<(i32, i32)>,
    pub min: Option<Size>,
    pub max: Option<Size>,
    pub base: Option<Size>,
    pub increment: Option<Size>,
    pub aspect: Option<AspectRange>,
    pub gravity: Option<u32>,
}

impl SizeHints {
    pub(crate) fn parse(words: &[u32]) -> Self {
        let word = |index: usize| words.get(index).copied();
        let Some(flags) = word(0) else {
            return Self::default();
        };
        let has = |bit: u32| flags & bit != 0;
        let pair = |bit: u32, index: usize| {
            has(bit)
                .then(|| Some((word(index)?, word(index + 1)?)))
                .flatten()
        };
        let size = |bit: u32, index: usize| {
            pair(bit, index).map(|(width, height)| Size {
                width: non_negative(width),
                height: non_negative(height),
            })
        };
        let ratio = |numerator: u32, denominator: u32| {
            let ratio = Ratio {
                numerator: non_negative(numerator),
                denominator: non_negative(denominator),
            };
            (ratio.numerator > 0 && ratio.denominator > 0).then_some(ratio)
        };
        Self {
            position: pair(USER_POSITION | PROGRAM_POSITION, 1)
                .map(|(x, y)| (x.cast_signed(), y.cast_signed())),
            min: size(MIN_SIZE, 5),
            max: size(MAX_SIZE, 7).filter(|max| max.width > 0 && max.height > 0),
            increment: size(RESIZE_INCREMENT, 9).filter(|step| step.width > 0 && step.height > 0),
            aspect: pair(ASPECT, 11)
                .zip(pair(ASPECT, 13))
                .and_then(|(min, max)| {
                    Some(AspectRange {
                        min: ratio(min.0, min.1)?,
                        max: ratio(max.0, max.1)?,
                    })
                }),
            base: size(BASE_SIZE, 15),
            gravity: has(WIN_GRAVITY).then(|| word(17)).flatten(),
        }
    }

    #[must_use]
    pub fn min_size(&self) -> Option<Size> {
        self.min.or(self.base)
    }
}

fn non_negative(word: u32) -> u32 {
    word.cast_signed().max(0).cast_unsigned()
}

pub(crate) fn motif_decorated(words: &[u32]) -> bool {
    match (words.first(), words.get(2)) {
        (Some(flags), Some(decorations)) if flags & MOTIF_DECORATIONS != 0 => {
            decorations & (DECORATE_ALL | DECORATE_BORDER | DECORATE_TITLE) != 0
        }
        _ => true,
    }
}

pub(crate) fn decode_text(utf8: bool, bytes: &[u8]) -> String {
    let bytes = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
    if utf8 {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        bytes.iter().copied().map(char::from).collect()
    }
}
