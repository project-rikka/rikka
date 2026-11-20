use std::cell::{Ref, RefCell};
use std::rc::Rc;

use code_meaning::Meaning;
use rikka_barcode::Format;
use rikka_network::Unjoinable;
use rikka_types::{Point, Rect};

const SAME_PLACE: f32 = 0.3;

#[derive(Clone, Debug)]
pub(crate) struct Code {
    pub(crate) format: Format,
    pub(crate) text: String,
    pub(crate) meaning: Meaning,
    pub(crate) area: Rect,
}

impl Code {
    fn repeats(&self, other: &Code) -> bool {
        self.text == other.text && overlap(self.area, other.area) >= SAME_PLACE
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Framing {
    pub(crate) area: Rect,
    turn: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Scope {
    Screen,
    Area(Framing),
}

#[derive(Debug, Default)]
pub(crate) struct DecodedCode {
    pub(crate) codes: Vec<Code>,
    pub(crate) chosen: Option<usize>,
    screening: bool,
    pub(crate) screened: bool,
    pub(crate) framing: Option<Framing>,
    pub(crate) missed: Option<Rect>,
    pub(crate) trouble: Option<Unjoinable>,
    turns: u64,
}

impl DecodedCode {
    #[must_use]
    pub(crate) fn chosen(&self) -> Option<(usize, &Code)> {
        let index = self.chosen?;
        Some((index, self.codes.get(index)?))
    }

    #[must_use]
    pub(crate) fn looking(&self) -> bool {
        self.screening || self.framing.is_some()
    }

    #[must_use]
    pub(crate) fn at(&self, point: Point) -> Option<usize> {
        self.codes
            .iter()
            .enumerate()
            .filter(|(_, code)| code.area.contains(point))
            .min_by(|(_, one), (_, other)| {
                let size = |code: &Code| code.area.width() * code.area.height();
                size(one).total_cmp(&size(other))
            })
            .map(|(index, _)| index)
    }

    fn settled(&mut self, scope: Scope) -> bool {
        match scope {
            Scope::Screen if self.screening => {
                self.screening = false;
                self.screened = true;
                true
            }
            Scope::Area(framing) if self.framing == Some(framing) => {
                self.framing = None;
                true
            }
            Scope::Screen | Scope::Area(_) => false,
        }
    }

    fn index_or_insert(&mut self, code: Code) -> usize {
        match self
            .codes
            .iter()
            .position(|index_or_insert| index_or_insert.repeats(&code))
        {
            Some(index) => index,
            None => {
                self.codes.push(code);
                self.codes.len() - 1
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Codes(Rc<RefCell<DecodedCode>>);

impl Codes {
    #[must_use]
    pub(crate) fn read(&self) -> Ref<'_, DecodedCode> {
        self.0.borrow()
    }

    pub(crate) fn clear(&self) {
        *self.0.borrow_mut() = DecodedCode::default();
    }

    #[must_use]
    pub(crate) fn screen(&self) -> Option<Scope> {
        let mut decoded = self.0.borrow_mut();
        if decoded.screening || decoded.screened {
            return None;
        }
        decoded.screening = true;
        Some(Scope::Screen)
    }

    #[must_use]
    pub(crate) fn frame(&self, area: Rect) -> Scope {
        let mut decoded = self.0.borrow_mut();
        decoded.turns += 1;
        let framing = Framing {
            area,
            turn: decoded.turns,
        };
        decoded.framing = Some(framing);
        decoded.missed = None;
        Scope::Area(framing)
    }

    pub(crate) fn take_in(&self, scope: Scope, sighted: Vec<Code>) -> bool {
        let mut decoded = self.0.borrow_mut();
        if !decoded.settled(scope) {
            return false;
        }
        let indices: Vec<usize> = sighted
            .into_iter()
            .map(|code| decoded.index_or_insert(code))
            .collect();
        match scope {
            Scope::Screen if decoded.chosen.is_none() && decoded.codes.len() == 1 => {
                decoded.chosen = Some(0);
            }
            Scope::Screen => {}
            Scope::Area(framing) => match indices.first() {
                Some(&first) => decoded.chosen = Some(first),
                None => {
                    decoded.chosen = None;
                    decoded.missed = Some(framing.area);
                }
            },
        }
        decoded.trouble = None;
        true
    }

    pub(crate) fn failed(&self, scope: Scope) -> bool {
        let mut decoded = self.0.borrow_mut();
        if !decoded.settled(scope) {
            return false;
        }
        if let Scope::Area(framing) = scope {
            decoded.missed = Some(framing.area);
        }
        true
    }

    pub(crate) fn choose(&self, chosen: Option<usize>) {
        let mut decoded = self.0.borrow_mut();
        decoded.chosen = chosen.filter(|&index| index < decoded.codes.len());
        decoded.missed = None;
        decoded.trouble = None;
    }

    #[must_use]
    pub(crate) fn code(&self, index: usize) -> Option<Code> {
        self.0.borrow().codes.get(index).cloned()
    }

    pub(crate) fn troubled(&self, trouble: Unjoinable) {
        self.0.borrow_mut().trouble = Some(trouble);
    }
}

fn overlap(one: Rect, other: Rect) -> f32 {
    let Some(shared) = one.intersection(&other) else {
        return 0.0;
    };
    let area = |rect: Rect| rect.width() * rect.height();
    let union = area(one) + area(other) - area(shared);
    match union > 0.0 {
        true => area(shared) / union,
        false => 0.0,
    }
}
