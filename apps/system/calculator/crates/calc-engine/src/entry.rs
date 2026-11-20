use crate::format;

const MAX_DIGITS: usize = format::SIGNIFICANT_DIGITS;

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    integer: String,
    fraction: Option<String>,
    negative: bool,
}

impl Entry {
    pub(crate) fn zero() -> Self {
        Self {
            integer: String::from("0"),
            fraction: None,
            negative: false,
        }
    }

    pub(crate) fn push_digit(&mut self, digit: u8) {
        debug_assert!(digit <= 9, "a keypad only ever presses one of ten");
        if self.digits() >= MAX_DIGITS {
            return;
        }
        let digit = char::from(b'0' + digit);
        match &mut self.fraction {
            Some(fraction) => fraction.push(digit),
            None if self.integer == "0" => self.integer = String::from(digit),
            None => self.integer.push(digit),
        }
    }

    pub(crate) fn push_point(&mut self) {
        if self.fraction.is_none() {
            self.fraction = Some(String::new());
        }
    }

    pub(crate) fn backspace(&mut self) {
        match &mut self.fraction {
            Some(fraction) if !fraction.is_empty() => {
                fraction.pop();
            }
            Some(_) => self.fraction = None,
            None => {
                self.integer.pop();
                if self.integer.is_empty() {
                    self.integer = String::from("0");
                    self.negative = false;
                }
            }
        }
    }

    pub(crate) fn negate(&mut self) {
        self.negative = !self.negative;
    }

    pub(crate) fn value(&self) -> f64 {
        self.compose(str::to_owned)
            .parse()
            .expect("digits around at most one point parse as a number")
    }

    pub(crate) fn text(&self) -> String {
        self.compose(format::group)
    }

    fn compose(&self, render: impl FnOnce(&str) -> String) -> String {
        let mut composed = String::new();
        if self.negative {
            composed.push('-');
        }
        composed.push_str(&render(&self.integer));
        if let Some(fraction) = &self.fraction {
            composed.push('.');
            composed.push_str(fraction);
        }
        composed
    }

    fn digits(&self) -> usize {
        let integer = if self.integer == "0" {
            0
        } else {
            self.integer.len()
        };
        integer + self.fraction.as_ref().map_or(0, String::len)
    }
}
