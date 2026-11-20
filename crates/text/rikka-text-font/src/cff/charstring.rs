use crate::cff::blend::{Blend, apply_blend};
use crate::cff::charset::Charset;
use crate::cff::index::Index;
use crate::cff::operands::{Operands, STACK_LIMIT, TRANSIENT_SIZE};
use crate::cff::pen::Pen;
use crate::cff::standard_encoding::STANDARD_ENCODING;
use crate::outline::sink::OutlineSink;
use crate::parse::bytes::Bytes;

const CFF_STACK_LIMIT: usize = 48;
const SUBR_NESTING_LIMIT: u32 = 10;
const OPERATION_BUDGET: u32 = 1 << 20;

mod op {
    pub(super) const HSTEM: u8 = 1;
    pub(super) const VSTEM: u8 = 3;
    pub(super) const VMOVETO: u8 = 4;
    pub(super) const RLINETO: u8 = 5;
    pub(super) const HLINETO: u8 = 6;
    pub(super) const VLINETO: u8 = 7;
    pub(super) const RRCURVETO: u8 = 8;
    pub(super) const CALLSUBR: u8 = 10;
    pub(super) const RETURN: u8 = 11;
    pub(super) const ESCAPE: u8 = 12;
    pub(super) const ENDCHAR: u8 = 14;
    pub(super) const VSINDEX: u8 = 15;
    pub(super) const BLEND: u8 = 16;
    pub(super) const HSTEMHM: u8 = 18;
    pub(super) const HINTMASK: u8 = 19;
    pub(super) const CNTRMASK: u8 = 20;
    pub(super) const RMOVETO: u8 = 21;
    pub(super) const HMOVETO: u8 = 22;
    pub(super) const VSTEMHM: u8 = 23;
    pub(super) const RCURVELINE: u8 = 24;
    pub(super) const RLINECURVE: u8 = 25;
    pub(super) const VVCURVETO: u8 = 26;
    pub(super) const HHCURVETO: u8 = 27;
    pub(super) const SHORTINT: u8 = 28;
    pub(super) const CALLGSUBR: u8 = 29;
    pub(super) const VHCURVETO: u8 = 30;
    pub(super) const HVCURVETO: u8 = 31;
    pub(super) const FIXED: u8 = 255;

    pub(super) const AND: u8 = 3;
    pub(super) const OR: u8 = 4;
    pub(super) const NOT: u8 = 5;
    pub(super) const ABS: u8 = 9;
    pub(super) const ADD: u8 = 10;
    pub(super) const SUB: u8 = 11;
    pub(super) const DIV: u8 = 12;
    pub(super) const NEG: u8 = 14;
    pub(super) const EQ: u8 = 15;
    pub(super) const DROP: u8 = 18;
    pub(super) const PUT: u8 = 20;
    pub(super) const GET: u8 = 21;
    pub(super) const IFELSE: u8 = 22;
    pub(super) const RANDOM: u8 = 23;
    pub(super) const MUL: u8 = 24;
    pub(super) const SQRT: u8 = 26;
    pub(super) const DUP: u8 = 27;
    pub(super) const EXCH: u8 = 28;
    pub(super) const INDEX: u8 = 29;
    pub(super) const ROLL: u8 = 30;
    pub(super) const HFLEX: u8 = 34;
    pub(super) const FLEX: u8 = 35;
    pub(super) const HFLEX1: u8 = 36;
    pub(super) const FLEX1: u8 = 37;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Seac<'a> {
    pub(crate) charstrings: Index<'a>,
    pub(crate) charset: Charset<'a>,
}

impl<'a> Seac<'a> {
    fn charstring(&self, code: f64) -> Option<&'a [u8]> {
        if !(0.0..256.0).contains(&code) {
            return None;
        }
        let sid = STANDARD_ENCODING[code as usize];
        let glyph = self.charset.glyph(u16::from(sid), self.charstrings.len())?;
        self.charstrings.get(u32::from(glyph.0))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Program<'a, 'c> {
    pub(crate) global_subrs: Index<'a>,
    pub(crate) local_subrs: Index<'a>,
    pub(crate) seac: Option<Seac<'a>>,
    pub(crate) blend: Option<Blend<'a, 'c>>,
    pub(crate) vsindex: u16,
    pub(crate) is_cff2: bool,
}

impl Program<'_, '_> {
    pub(crate) fn execute<S: OutlineSink>(
        &self,
        charstring: &[u8],
        transform: Option<[f32; 4]>,
        sink: &mut S,
        operands: &mut Operands,
    ) -> Option<Option<f64>> {
        operands.transient = [0.0; TRANSIENT_SIZE];
        let mut machine = Machine {
            program: self,
            pen: Pen::new(sink, transform),
            ops: operands,
            len: 0,
            limit: if self.is_cff2 {
                STACK_LIMIT
            } else {
                CFF_STACK_LIMIT
            },
            x: 0.0,
            y: 0.0,
            stems: 0,
            seen_width: self.is_cff2,
            width: None,
            blend_ready: false,
            vsindex: self.vsindex,
            budget: OPERATION_BUDGET,
        };
        machine.run(charstring, 0)?;
        machine.pen.close();
        Some(machine.width)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flow {
    Return,
    End,
}

struct Machine<'p, 'a, 'c, 's, 'o, S> {
    program: &'p Program<'a, 'c>,
    pen: Pen<'s, S>,
    ops: &'o mut Operands,
    len: usize,
    limit: usize,
    x: f64,
    y: f64,
    stems: usize,
    seen_width: bool,
    width: Option<f64>,
    blend_ready: bool,
    vsindex: u16,
    budget: u32,
}

impl<S: OutlineSink> Machine<'_, '_, '_, '_, '_, S> {
    #[inline(always)]
    fn push_operand(&mut self, value: f64) -> Option<()> {
        if self.len >= self.limit {
            return None;
        }
        self.ops.stack[self.len] = value;
        self.len += 1;
        Some(())
    }

    #[inline]
    fn push(&mut self, value: f64) -> Option<()> {
        if !value.is_finite() {
            return None;
        }
        self.push_operand(value)
    }

    #[inline]
    fn pop(&mut self) -> Option<f64> {
        self.len = self.len.checked_sub(1)?;
        Some(self.ops.stack[self.len])
    }

    #[inline]
    fn arguments_start(&mut self, has_width: bool) -> usize {
        if self.seen_width {
            return 0;
        }
        self.seen_width = true;
        if has_width && self.len > 0 {
            self.width = Some(self.ops.stack[0]);
            1
        } else {
            0
        }
    }

    fn run(&mut self, code: &[u8], depth: u32) -> Option<Flow> {
        let mut position = 0;
        while position < code.len() {
            self.budget = self.budget.checked_sub(1)?;
            let b0 = code.u8_at(position)?;
            position += 1;
            match b0 {
                32..=246 => self.push_operand(f64::from(i32::from(b0) - 139))?,
                247..=250 => {
                    let b1 = code.u8_at(position)?;
                    position += 1;
                    self.push_operand(f64::from(
                        (i32::from(b0) - 247) * 256 + i32::from(b1) + 108,
                    ))?;
                }
                251..=254 => {
                    let b1 = code.u8_at(position)?;
                    position += 1;
                    self.push_operand(f64::from(
                        -(i32::from(b0) - 251) * 256 - i32::from(b1) - 108,
                    ))?;
                }
                op::SHORTINT => {
                    let value = code.i16_at(position)?;
                    position += 2;
                    self.push_operand(f64::from(value))?;
                }
                op::FIXED => {
                    let value = code.i32_at(position)?;
                    position += 4;
                    self.push_operand(f64::from(value) / 65536.0)?;
                }
                op::HSTEM | op::VSTEM | op::HSTEMHM | op::VSTEMHM => {
                    let start = self.arguments_start(self.len % 2 == 1);
                    self.stems += (self.len - start) / 2;
                    self.len = 0;
                }
                op::HINTMASK | op::CNTRMASK => {
                    let start = self.arguments_start(self.len % 2 == 1);
                    self.stems += (self.len - start) / 2;
                    self.len = 0;
                    position = position.checked_add(self.stems.div_ceil(8))?;
                    if position > code.len() {
                        return None;
                    }
                }
                op::RMOVETO => {
                    let start = self.arguments_start(self.len > 2);
                    if self.len < start + 2 {
                        return None;
                    }
                    self.x += self.ops.stack[self.len - 2];
                    self.y += self.ops.stack[self.len - 1];
                    self.pen.move_to(self.x, self.y);
                    self.len = 0;
                }
                op::HMOVETO | op::VMOVETO => {
                    let start = self.arguments_start(self.len > 1);
                    if self.len < start + 1 {
                        return None;
                    }
                    let delta = self.ops.stack[self.len - 1];
                    if b0 == op::HMOVETO {
                        self.x += delta;
                    } else {
                        self.y += delta;
                    }
                    self.pen.move_to(self.x, self.y);
                    self.len = 0;
                }
                op::RLINETO => {
                    self.lines(0, self.len);
                    self.len = 0;
                }
                op::HLINETO | op::VLINETO => {
                    let mut horizontal = b0 == op::HLINETO;
                    for index in 0..self.len {
                        if horizontal {
                            self.x += self.ops.stack[index];
                        } else {
                            self.y += self.ops.stack[index];
                        }
                        self.pen.line_to(self.x, self.y);
                        horizontal = !horizontal;
                    }
                    self.len = 0;
                }
                op::RRCURVETO => {
                    self.curves(0, self.len);
                    self.len = 0;
                }
                op::RCURVELINE => {
                    if self.len >= 2 {
                        let end = self.curves(0, self.len - 2);
                        self.lines(end, self.len);
                    }
                    self.len = 0;
                }
                op::RLINECURVE => {
                    if self.len >= 6 {
                        let end = self.lines(0, self.len - 6);
                        self.curves(end, self.len);
                    }
                    self.len = 0;
                }
                op::HHCURVETO => {
                    self.hh_curves();
                    self.len = 0;
                }
                op::VVCURVETO => {
                    self.vv_curves();
                    self.len = 0;
                }
                op::HVCURVETO | op::VHCURVETO => {
                    self.alternating_curves(b0 == op::HVCURVETO);
                    self.len = 0;
                }
                op::CALLSUBR | op::CALLGSUBR => {
                    let subrs = if b0 == op::CALLSUBR {
                        self.program.local_subrs
                    } else {
                        self.program.global_subrs
                    };
                    let index = self.pop()? + f64::from(subrs.subr_bias());
                    if !(0.0..f64::from(u32::MAX)).contains(&index) || depth >= SUBR_NESTING_LIMIT {
                        return None;
                    }
                    let subr = subrs.get(index as u32)?;
                    if self.run(subr, depth + 1)? == Flow::End {
                        return Some(Flow::End);
                    }
                }
                op::RETURN if !self.program.is_cff2 => return Some(Flow::Return),
                op::ENDCHAR if !self.program.is_cff2 => {
                    let start = self.arguments_start(self.len == 1 || self.len == 5);
                    if self.len - start == 4 {
                        self.seac(start, depth)?;
                    }
                    self.len = 0;
                    return Some(Flow::End);
                }
                op::VSINDEX if self.program.is_cff2 => {
                    let value = self.pop()?;
                    if !(0.0..65536.0).contains(&value) {
                        return None;
                    }
                    self.vsindex = value as u16;
                    self.blend_ready = false;
                    self.len = 0;
                }
                op::BLEND if self.program.is_cff2 => {
                    self.regions()?;
                    self.len = apply_blend(&mut self.ops.stack, self.len, &self.ops.regions)?;
                }
                op::ESCAPE => {
                    let b1 = code.u8_at(position)?;
                    position += 1;
                    self.escape(b1)?;
                }
                _ => self.len = 0,
            }
        }
        Some(Flow::Return)
    }

    fn regions(&mut self) -> Option<()> {
        if !self.blend_ready {
            self.ops.regions.prepare(self.program.blend, self.vsindex)?;
            self.blend_ready = true;
        }
        Some(())
    }

    fn lines(&mut self, mut index: usize, end: usize) -> usize {
        while index + 2 <= end {
            self.x += self.ops.stack[index];
            self.y += self.ops.stack[index + 1];
            self.pen.line_to(self.x, self.y);
            index += 2;
        }
        index
    }

    fn curves(&mut self, mut index: usize, end: usize) -> usize {
        while index + 6 <= end {
            let s = &self.ops.stack[index..index + 6];
            self.curve(s[0], s[1], s[2], s[3], s[4], s[5]);
            index += 6;
        }
        index
    }

    #[inline]
    fn curve(&mut self, dx1: f64, dy1: f64, dx2: f64, dy2: f64, dx3: f64, dy3: f64) {
        let x1 = self.x + dx1;
        let y1 = self.y + dy1;
        let x2 = x1 + dx2;
        let y2 = y1 + dy2;
        self.x = x2 + dx3;
        self.y = y2 + dy3;
        self.pen.curve_to(x1, y1, x2, y2, self.x, self.y);
    }

    fn hh_curves(&mut self) {
        let mut index = 0;
        let mut dy1 = 0.0;
        if self.len % 2 == 1 {
            dy1 = self.ops.stack[0];
            index = 1;
        }
        while index + 4 <= self.len {
            let s = &self.ops.stack[index..index + 4];
            self.curve(s[0], dy1, s[1], s[2], s[3], 0.0);
            dy1 = 0.0;
            index += 4;
        }
    }

    fn vv_curves(&mut self) {
        let mut index = 0;
        let mut dx1 = 0.0;
        if self.len % 2 == 1 {
            dx1 = self.ops.stack[0];
            index = 1;
        }
        while index + 4 <= self.len {
            let s = &self.ops.stack[index..index + 4];
            self.curve(dx1, s[0], s[1], s[2], 0.0, s[3]);
            dx1 = 0.0;
            index += 4;
        }
    }

    fn alternating_curves(&mut self, mut horizontal: bool) {
        let mut index = 0;
        while index + 4 <= self.len {
            let s = &self.ops.stack[index..index + 4];
            let last = if self.len - index == 5 {
                self.ops.stack[index + 4]
            } else {
                0.0
            };
            if horizontal {
                self.curve(s[0], 0.0, s[1], s[2], last, s[3]);
            } else {
                self.curve(0.0, s[0], s[1], s[2], s[3], last);
            }
            horizontal = !horizontal;
            index += 4;
        }
    }

    fn seac(&mut self, start: usize, depth: u32) -> Option<()> {
        if depth >= SUBR_NESTING_LIMIT {
            return None;
        }
        let seac = self.program.seac?;
        let s = &self.ops.stack[start..start + 4];
        let (adx, ady, base_code, accent_code) = (s[0], s[1], s[2], s[3]);
        let accent = seac.charstring(accent_code)?;
        let base = seac.charstring(base_code)?;
        let (x, y) = (self.x, self.y);
        let width = self.width;
        for (charstring, origin) in [(accent, (adx, ady)), (base, (x, y))] {
            self.pen.close();
            self.x = origin.0;
            self.y = origin.1;
            self.pen.move_to(self.x, self.y);
            self.len = 0;
            self.stems = 0;
            self.seen_width = false;
            self.run(charstring, depth + 1)?;
        }
        self.pen.close();
        self.width = width;
        self.seen_width = true;
        Some(())
    }

    fn escape(&mut self, operator: u8) -> Option<()> {
        match operator {
            op::HFLEX => {
                if self.len >= 7 {
                    let s: [f64; 7] = self.ops.stack[..7].try_into().ok()?;
                    let start_y = self.y;
                    self.curve(s[0], 0.0, s[1], s[2], s[3], 0.0);
                    self.curve(s[4], 0.0, s[5], start_y - self.y, s[6], 0.0);
                }
                self.len = 0;
            }
            op::FLEX => {
                if self.len >= 13 {
                    self.curves(0, 12);
                }
                self.len = 0;
            }
            op::HFLEX1 => {
                if self.len >= 9 {
                    let s: [f64; 9] = self.ops.stack[..9].try_into().ok()?;
                    let start_y = self.y;
                    self.curve(s[0], s[1], s[2], s[3], s[4], 0.0);
                    let dy6 = start_y - self.y - s[7];
                    self.curve(s[5], 0.0, s[6], s[7], s[8], dy6);
                }
                self.len = 0;
            }
            op::FLEX1 => {
                if self.len >= 11 {
                    let s: [f64; 11] = self.ops.stack[..11].try_into().ok()?;
                    let (start_x, start_y) = (self.x, self.y);
                    let dx: f64 = (0..5).map(|i| s[i * 2]).sum();
                    let dy: f64 = (0..5).map(|i| s[i * 2 + 1]).sum();
                    self.curve(s[0], s[1], s[2], s[3], s[4], s[5]);
                    let (end_x, end_y) = if dx.abs() > dy.abs() {
                        (start_x + dx + s[10], start_y)
                    } else {
                        (start_x, start_y + dy + s[10])
                    };
                    let x1 = self.x + s[6];
                    let y1 = self.y + s[7];
                    let x2 = x1 + s[8];
                    let y2 = y1 + s[9];
                    self.x = end_x;
                    self.y = end_y;
                    self.pen.curve_to(x1, y1, x2, y2, end_x, end_y);
                }
                self.len = 0;
            }
            _ => self.arithmetic(operator)?,
        }
        Some(())
    }

    fn arithmetic(&mut self, operator: u8) -> Option<()> {
        match operator {
            op::ABS | op::NEG | op::NOT | op::SQRT => {
                let a = self.pop()?;
                let result = match operator {
                    op::ABS => a.abs(),
                    op::NEG => -a,
                    op::NOT => f64::from(u8::from(a == 0.0)),
                    _ => a.max(0.0).sqrt(),
                };
                self.push(result)?;
            }
            op::ADD | op::SUB | op::MUL | op::DIV | op::AND | op::OR | op::EQ => {
                let b = self.pop()?;
                let a = self.pop()?;
                let result = match operator {
                    op::ADD => a + b,
                    op::SUB => a - b,
                    op::MUL => a * b,
                    op::DIV if b != 0.0 => a / b,
                    op::DIV => 0.0,
                    op::AND => f64::from(u8::from(a != 0.0 && b != 0.0)),
                    op::OR => f64::from(u8::from(a != 0.0 || b != 0.0)),
                    _ => f64::from(u8::from(a == b)),
                };
                self.push(result)?;
            }
            op::DROP => {
                self.pop()?;
            }
            op::PUT => {
                let index = self.pop()?;
                let value = self.pop()?;
                if (0.0..TRANSIENT_SIZE as f64).contains(&index) {
                    self.ops.transient[index as usize] = value;
                }
            }
            op::GET => {
                let index = self.pop()?;
                let value = if (0.0..TRANSIENT_SIZE as f64).contains(&index) {
                    self.ops.transient[index as usize]
                } else {
                    0.0
                };
                self.push(value)?;
            }
            op::IFELSE => {
                let v2 = self.pop()?;
                let v1 = self.pop()?;
                let s2 = self.pop()?;
                let s1 = self.pop()?;
                self.push(if v1 <= v2 { s1 } else { s2 })?;
            }
            op::RANDOM => self.push(0.5)?,
            op::DUP => {
                let a = self.pop()?;
                self.push(a)?;
                self.push(a)?;
            }
            op::EXCH => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(b)?;
                self.push(a)?;
            }
            op::INDEX => {
                let index = self.pop()?;
                let top = self.len.checked_sub(1)?;
                let offset = if index < 0.0 { 0 } else { index as usize };
                let value = self.ops.stack[top.checked_sub(offset)?];
                self.push(value)?;
            }
            op::ROLL => {
                let shift = self.pop()?;
                let count = self.pop()?;
                if !(0.0..=self.len as f64).contains(&count) {
                    return None;
                }
                let count = count as usize;
                if count > 0 {
                    let items = &mut self.ops.stack[self.len - count..self.len];
                    let shift = (shift as i64).rem_euclid(count as i64) as usize;
                    items.rotate_right(shift);
                }
            }
            _ => self.len = 0,
        }
        Some(())
    }
}
