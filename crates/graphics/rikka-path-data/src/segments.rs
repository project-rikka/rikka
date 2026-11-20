use std::collections::VecDeque;

use rikka_types::Point;

use crate::arc::Arc;
use crate::error::{Error, ErrorKind};
use crate::lexer::Lexer;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    MoveTo(Point),
    LineTo(Point),
    QuadTo(Point, Point),
    CubicTo(Point, Point, Point),
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Reflection {
    Cubic(Point),
    Quad(Point),
}

#[derive(Clone, Debug)]
pub struct Segments<'a> {
    lexer: Lexer<'a>,
    command: Option<u8>,
    moved: bool,
    current: Point,
    start: Point,
    reflection: Option<Reflection>,
    pending: VecDeque<Segment>,
    ended: bool,
}

#[must_use]
pub fn segments(text: &str) -> Segments<'_> {
    Segments {
        lexer: Lexer::new(text),
        command: None,
        moved: false,
        current: Point::ZERO,
        start: Point::ZERO,
        reflection: None,
        pending: VecDeque::new(),
        ended: false,
    }
}

impl Iterator for Segments<'_> {
    type Item = Result<Segment, Error>;

    fn next(&mut self) -> Option<Result<Segment, Error>> {
        if let Some(segment) = self.pending.pop_front() {
            return Some(Ok(segment));
        }
        while !self.ended {
            match self.read() {
                Ok(Some(segment)) => return Some(Ok(segment)),
                Ok(None) => {
                    if let Some(segment) = self.pending.pop_front() {
                        return Some(Ok(segment));
                    }
                }
                Err(error) => {
                    self.ended = true;
                    return Some(Err(error));
                }
            }
        }
        None
    }
}

impl Segments<'_> {
    fn read(&mut self) -> Result<Option<Segment>, Error> {
        self.lexer.skip_separators();
        let Some(next) = self.lexer.peek() else {
            self.ended = true;
            return Ok(None);
        };
        if !self.moved && !matches!(next, b'M' | b'm') {
            return Err(self.lexer.fault(ErrorKind::NoMoveFirst));
        }
        if next.is_ascii_alphabetic() {
            self.command = Some(next);
            self.lexer.advance();
        }
        let command = match self.command {
            Some(command) if next.is_ascii_alphabetic() || self.lexer.starts_number() => command,
            _ => {
                let found = self.lexer.peek_char().unwrap_or_default();
                return Err(self.lexer.fault(ErrorKind::UnknownCommand(found)));
            }
        };
        let origin = match command.is_ascii_lowercase() {
            true => self.current,
            false => Point::ZERO,
        };
        let mut reflection = None;
        let segment = match command.to_ascii_uppercase() {
            b'M' => {
                let to = origin + self.point()?;
                self.current = to;
                self.start = to;
                self.moved = true;
                self.command = Some(match command {
                    b'm' => b'l',
                    _ => b'L',
                });
                Some(Segment::MoveTo(to))
            }
            b'L' => {
                self.current = origin + self.point()?;
                Some(Segment::LineTo(self.current))
            }
            b'H' => {
                self.current = Point::new(origin.x + self.lexer.number()?, self.current.y);
                Some(Segment::LineTo(self.current))
            }
            b'V' => {
                self.current = Point::new(self.current.x, origin.y + self.lexer.number()?);
                Some(Segment::LineTo(self.current))
            }
            b'C' => {
                let first = origin + self.point()?;
                let second = origin + self.point()?;
                self.current = origin + self.point()?;
                reflection = Some(Reflection::Cubic(second));
                Some(Segment::CubicTo(first, second, self.current))
            }
            b'S' => {
                let first = match self.reflection {
                    Some(Reflection::Cubic(control)) => self.current * 2.0 - control,
                    _ => self.current,
                };
                let second = origin + self.point()?;
                self.current = origin + self.point()?;
                reflection = Some(Reflection::Cubic(second));
                Some(Segment::CubicTo(first, second, self.current))
            }
            b'Q' => {
                let control = origin + self.point()?;
                self.current = origin + self.point()?;
                reflection = Some(Reflection::Quad(control));
                Some(Segment::QuadTo(control, self.current))
            }
            b'T' => {
                let control = match self.reflection {
                    Some(Reflection::Quad(control)) => self.current * 2.0 - control,
                    _ => self.current,
                };
                self.current = origin + self.point()?;
                reflection = Some(Reflection::Quad(control));
                Some(Segment::QuadTo(control, self.current))
            }
            b'A' => {
                let radii = Point::new(self.lexer.number()?, self.lexer.number()?);
                let rotation = self.lexer.number()?;
                let large = self.lexer.flag()?;
                let sweep = self.lexer.flag()?;
                let to = origin + self.point()?;
                let arc = Arc {
                    from: self.current,
                    radii,
                    rotation,
                    large,
                    sweep,
                    to,
                };
                self.current = to;
                self.pending.extend(arc.segments());
                None
            }
            b'Z' => {
                self.current = self.start;
                self.command = None;
                Some(Segment::Close)
            }
            other => {
                return Err(Error {
                    offset: self.lexer.offset() - 1,
                    kind: ErrorKind::UnknownCommand(char::from(other)),
                });
            }
        };
        self.reflection = reflection;
        Ok(segment)
    }

    fn point(&mut self) -> Result<Point, Error> {
        Ok(Point::new(self.lexer.number()?, self.lexer.number()?))
    }
}
