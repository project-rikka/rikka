use rikka_model_core::glam::Vec3;
use rikka_model_core::{Error, ImportCtx, Result, Stage, Warning};

use crate::facet::{Document, Facet, Solid};

pub(crate) fn parse(bytes: &[u8], ctx: &mut ImportCtx<'_>) -> Result<Document> {
    let text = String::from_utf8_lossy(bytes);
    let mut parser = Parser {
        document: Document::default(),
        state: State::Outside,
        normal: Vec3::ZERO,
        polygon: Vec::new(),
        polygons: 0,
        ctx,
    };
    let mut read = 0;
    for (index, line) in text.lines().enumerate() {
        if index % 262_144 == 0 {
            parser.ctx.checkpoint()?;
            parser
                .ctx
                .report(Stage::Parsing, 0.7 * read as f32 / text.len().max(1) as f32);
        }
        read += line.len() + 1;
        parser.line(line, index + 1)?;
    }
    parser.finish()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Outside,
    Solid,
    Facet,
    Loop,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Keyword {
    Solid,
    EndSolid,
    Facet,
    Normal,
    Outer,
    Loop,
    Vertex,
    EndLoop,
    EndFacet,
}

const KEYWORDS: [(&str, Keyword); 9] = [
    ("solid", Keyword::Solid),
    ("endsolid", Keyword::EndSolid),
    ("facet", Keyword::Facet),
    ("normal", Keyword::Normal),
    ("outer", Keyword::Outer),
    ("loop", Keyword::Loop),
    ("vertex", Keyword::Vertex),
    ("endloop", Keyword::EndLoop),
    ("endfacet", Keyword::EndFacet),
];

fn keyword(word: &str) -> Option<Keyword> {
    KEYWORDS
        .iter()
        .find(|(spelled, _)| spelled.eq_ignore_ascii_case(word))
        .map(|&(_, keyword)| keyword)
}

struct Parser<'c, 'a> {
    document: Document,
    state: State,
    normal: Vec3,
    polygon: Vec<Vec3>,
    polygons: usize,
    ctx: &'c mut ImportCtx<'a>,
}

impl Parser<'_, '_> {
    fn line(&mut self, line: &str, number: usize) -> Result<()> {
        let mut words = line.split_ascii_whitespace();
        while let Some(word) = words.next() {
            match (self.state, keyword(word)) {
                (State::Outside, Some(Keyword::Solid)) => {
                    let name = line.trim_start()[word.len()..].trim();
                    self.document.solids.push(Solid {
                        name: name.to_owned(),
                        facets: Vec::new(),
                    });
                    self.state = State::Solid;
                    return Ok(());
                }
                (State::Solid, Some(Keyword::EndSolid)) => {
                    self.state = State::Outside;
                    return Ok(());
                }
                (State::Solid, Some(Keyword::Facet)) => {
                    self.normal = Vec3::ZERO;
                    self.polygon.clear();
                    self.state = State::Facet;
                }
                (State::Facet, Some(Keyword::Normal)) => self.normal = vector(&mut words, number)?,
                (State::Facet, Some(Keyword::Outer)) => {}
                (State::Facet, Some(Keyword::Loop)) => self.state = State::Loop,
                (State::Loop | State::Facet, Some(Keyword::Vertex)) => {
                    let corner = vector(&mut words, number)?;
                    self.polygon.push(corner);
                }
                (State::Loop, Some(Keyword::EndLoop)) => self.state = State::Facet,
                (State::Facet, Some(Keyword::EndFacet)) => {
                    self.close();
                    self.state = State::Solid;
                }
                _ => {
                    return Err(Error::Damaged(format!(
                        "line {number}: unexpected keyword \"{word}\""
                    )));
                }
            }
        }
        Ok(())
    }

    fn close(&mut self) {
        let solid = self
            .document
            .solids
            .last_mut()
            .expect("facet outside a solid");
        match self.polygon.len() {
            0..=2 => {
                self.ctx.warn(Warning::Repaired(format!(
                    "skipping facet of solid \"{}\" with {} vertices",
                    solid.name,
                    self.polygon.len()
                )));
                return;
            }
            3 => {}
            _ => self.polygons += 1,
        }
        let first = self.polygon[0];
        for pair in self.polygon[1..].windows(2) {
            solid.facets.push(Facet {
                normal: self.normal,
                corners: [first, pair[0], pair[1]],
                color: None,
            });
        }
    }

    fn finish(mut self) -> Result<Document> {
        if self.state != State::Outside {
            self.ctx
                .warn(Warning::Repaired("unterminated solid".to_owned()));
            if matches!(self.state, State::Facet | State::Loop) {
                self.close();
            }
        }
        if self.polygons > 0 {
            self.ctx.warn(Warning::Repaired(format!(
                "{} facets have more than three corners and are cut into triangles",
                self.polygons
            )));
        }
        if self.document.solids.is_empty() {
            return Err(Error::Damaged("no solid in file".to_owned()));
        }
        Ok(self.document)
    }
}

fn vector<'a>(words: &mut impl Iterator<Item = &'a str>, number: usize) -> Result<Vec3> {
    let mut component = || -> Result<f32> {
        let word = words
            .next()
            .ok_or_else(|| Error::Damaged(format!("line {number}: expected three numbers")))?;
        match word.parse::<f32>() {
            Ok(value) if value.is_finite() => Ok(value),
            _ => Err(Error::Damaged(format!(
                "line {number}: \"{word}\" is not a finite number"
            ))),
        }
    };
    Ok(Vec3::new(component()?, component()?, component()?))
}
