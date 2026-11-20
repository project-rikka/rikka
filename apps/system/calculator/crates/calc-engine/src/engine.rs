use crate::error::Error;
use crate::expression::Expression;
use crate::format;
use crate::key::Key;
use crate::operand::Operand;
use crate::operator::Operator;
use crate::unary::Unary;

#[derive(Debug)]
pub struct Engine {
    accumulator: f64,
    pending: Option<Operator>,
    operand: Operand,
    awaiting_operand: bool,
    repeat: Option<(Operator, f64)>,
    expression: Expression,
    error: Option<Error>,
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            accumulator: 0.0,
            pending: None,
            operand: Operand::zero(),
            awaiting_operand: false,
            repeat: None,
            expression: Expression::default(),
            error: None,
        }
    }
}

impl Engine {
    pub fn display(&self) -> String {
        self.operand.text()
    }

    pub fn display_ungrouped(&self) -> String {
        self.display().replace(format::SEPARATOR, "")
    }

    pub fn expression(&self) -> String {
        self.expression.text()
    }

    pub fn error(&self) -> Option<Error> {
        self.error
    }

    pub fn accepts(&self, key: Key) -> bool {
        self.error.is_none() || matches!(key, Key::Clear | Key::ClearEntry)
    }

    pub fn press(&mut self, key: Key) {
        if !self.accepts(key) {
            return;
        }
        match key {
            Key::Digit(digit) => self.digit(digit),
            Key::Point => self.point(),
            Key::Operator(operator) => self.operator(operator),
            Key::Unary(operation) => self.unary(operation),
            Key::Percent => self.percent(),
            Key::Backspace => self.operand.backspace(),
            Key::ClearEntry => self.clear_entry(),
            Key::Clear => self.clear(),
            Key::Equals => self.equals(),
        }
    }

    fn digit(&mut self, digit: u8) {
        self.begin_operand();
        self.operand.push_digit(digit);
    }

    fn point(&mut self) {
        self.begin_operand();
        self.operand.push_point();
    }

    fn operator(&mut self, operator: Operator) {
        if self.awaiting_operand {
            self.pending = Some(operator);
            self.expression.replace_operator(operator.symbol());
            return;
        }

        self.restart_after_equals();
        let left = match self.pending {
            Some(pending) => match pending.apply(self.accumulator, self.operand.value()) {
                Ok(result) => result,
                Err(error) => return self.fail(error),
            },
            None => self.operand.value(),
        };

        let display = self.operand.text();
        self.accumulator = left;
        self.operand = Operand::Value(left);
        self.expression.push_operator(display, operator.symbol());
        self.pending = Some(operator);
        self.awaiting_operand = true;
    }

    fn unary(&mut self, operation: Unary) {
        if operation == Unary::Negate && self.operand.negate_typed() {
            return;
        }

        self.restart_after_equals();
        match operation.apply(self.operand.value()) {
            Ok(result) => {
                let display = self.operand.text();
                self.expression.wrap(operation.function(), display);
                self.operand = Operand::Value(result);
                self.awaiting_operand = false;
            }
            Err(error) => self.fail(error),
        }
    }

    fn percent(&mut self) {
        self.restart_after_equals();

        let value = self.operand.value();
        let result = match self.pending {
            Some(operator) if operator.percent_is_relative() => self.accumulator * value / 100.0,
            Some(_) => value / 100.0,
            None => 0.0,
        };

        match Error::finite(result) {
            Ok(result) => {
                self.operand = Operand::Value(result);
                self.expression.set_operand(format::number(result));
                self.awaiting_operand = false;
            }
            Err(error) => self.fail(error),
        }
    }

    fn equals(&mut self) {
        match (self.pending.take(), self.repeat) {
            (Some(operator), _) => {
                let right = self.operand.value();
                match operator.apply(self.accumulator, right) {
                    Ok(result) => {
                        let display = self.operand.text();
                        self.expression.equals(display);
                        self.set_result(result);
                        self.repeat = Some((operator, right));
                    }
                    Err(error) => self.fail(error),
                }
            }
            (None, Some((operator, right))) => match operator.apply(self.operand.value(), right) {
                Ok(result) => {
                    let left = self.operand.text();
                    self.expression
                        .set_repeated(left, operator.symbol(), format::number(right));
                    self.set_result(result);
                }
                Err(error) => self.fail(error),
            },
            (None, None) => {
                let value = self.operand.value();
                self.expression.equals(self.operand.text());
                self.set_result(value);
            }
        }
    }

    fn clear_entry(&mut self) {
        if self.error.is_some() {
            self.clear();
            return;
        }
        self.operand = Operand::zero();
        self.expression.discard_operand();
        self.awaiting_operand = false;
    }

    fn clear(&mut self) {
        *self = Self::default();
    }

    fn begin_operand(&mut self) {
        self.restart_after_equals();
        self.expression.discard_operand();
        self.awaiting_operand = false;
    }

    fn restart_after_equals(&mut self) {
        if self.repeat.take().is_some() {
            self.expression.clear();
        }
    }

    fn set_result(&mut self, result: f64) {
        self.accumulator = result;
        self.operand = Operand::Value(result);
        self.awaiting_operand = false;
    }

    fn fail(&mut self, error: Error) {
        self.error = Some(error);
    }
}
