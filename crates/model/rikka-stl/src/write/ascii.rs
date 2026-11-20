use std::fmt::{self, Display};

use rikka_model_core::glam::Vec3;

use crate::facet::Solid;

pub(crate) fn write(solids: &[Solid]) -> Vec<u8> {
    let mut text = String::new();
    write_solids(&mut text, solids).expect("formatting into a String is infallible");
    text.into_bytes()
}

fn write_solids(text: &mut impl fmt::Write, solids: &[Solid]) -> fmt::Result {
    for solid in solids {
        let name = name(&solid.name);
        writeln!(text, "solid {name}")?;
        for facet in &solid.facets {
            writeln!(text, "  facet normal {}", Point(facet.normal))?;
            text.write_str("    outer loop\n")?;
            for corner in facet.corners {
                writeln!(text, "      vertex {}", Point(corner))?;
            }
            text.write_str("    endloop\n  endfacet\n")?;
        }
        writeln!(text, "endsolid {name}")?;
    }
    Ok(())
}

fn name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(
            |character| match character.is_ascii_graphic() || character == ' ' {
                true => character,
                false => '_',
            },
        )
        .collect();
    match cleaned.trim() {
        "" => "rikka".to_owned(),
        trimmed => trimmed.to_owned(),
    }
}

struct Point(Vec3);

impl Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [x, y, z] = self.0.to_array().map(Scientific);
        write!(f, "{x} {y} {z}")
    }
}

struct Scientific(f32);

impl Display for Scientific {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let written = format!("{:.8e}", self.0);
        let (mantissa, exponent) = written
            .split_once('e')
            .expect("`{:e}` output has an exponent");
        let exponent: i32 = exponent.parse().expect("`{:e}` exponent is an integer");
        let sign = if exponent < 0 { '-' } else { '+' };
        write!(f, "{mantissa}e{sign}{:02}", exponent.abs())
    }
}
