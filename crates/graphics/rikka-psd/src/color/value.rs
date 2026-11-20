use crate::color::convert;
use crate::descriptor::{Descriptor, Unit, Value};
use crate::error::Result;
use crate::io::{Reader, Writer};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    Rgb { r: f64, g: f64, b: f64 },
    Hsb { h: f64, s: f64, b: f64 },
    Cmyk { c: f64, m: f64, y: f64, k: f64 },
    Lab { l: f64, a: f64, b: f64 },
    Gray { k: f64 },
    Other { space: u16, values: [u16; 4] },
}

impl Default for Color {
    fn default() -> Self {
        Color::BLACK
    }
}

impl Color {
    pub const BLACK: Color = Color::Rgb {
        r: 0.0,
        g: 0.0,
        b: 0.0,
    };
    pub const WHITE: Color = Color::Rgb {
        r: 1.0,
        g: 1.0,
        b: 1.0,
    };

    pub fn rgb8(r: u8, g: u8, b: u8) -> Self {
        Color::Rgb {
            r: f64::from(r) / 255.0,
            g: f64::from(g) / 255.0,
            b: f64::from(b) / 255.0,
        }
    }

    pub fn to_rgb(&self) -> [f64; 3] {
        match *self {
            Color::Rgb { r, g, b } => [r, g, b],
            Color::Hsb { h, s, b } => convert::hsb_to_rgb(h, s, b),
            Color::Cmyk { c, m, y, k } => convert::cmyk_to_rgb(c, m, y, k),
            Color::Lab { l, a, b } => convert::lab_to_rgb(l, a, b),
            Color::Gray { k } => [1.0 - k; 3],
            Color::Other { .. } => [0.0; 3],
        }
    }

    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        let space = reader.u16()?;
        let mut raw = [0u16; 4];
        for value in &mut raw {
            *value = reader.u16()?;
        }
        let unit = |value: u16| f64::from(value) / 65535.0;
        Ok(match space {
            0 => Color::Rgb {
                r: unit(raw[0]),
                g: unit(raw[1]),
                b: unit(raw[2]),
            },
            1 => Color::Hsb {
                h: unit(raw[0]) * 360.0,
                s: unit(raw[1]),
                b: unit(raw[2]),
            },
            2 => Color::Cmyk {
                c: 1.0 - unit(raw[0]),
                m: 1.0 - unit(raw[1]),
                y: 1.0 - unit(raw[2]),
                k: 1.0 - unit(raw[3]),
            },
            7 => {
                let signed = |value: u16| f64::from(value as i16) / 100.0;
                Color::Lab {
                    l: f64::from(raw[0] as i16) / 100.0,
                    a: signed(raw[1]),
                    b: signed(raw[2]),
                }
            }
            8 => Color::Gray {
                k: f64::from(raw[0]) / 10000.0,
            },
            _ => Color::Other { space, values: raw },
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) {
        let unit = |value: f64| (value.clamp(0.0, 1.0) * 65535.0).round() as u16;
        let (space, values) = match *self {
            Color::Rgb { r, g, b } => (0, [unit(r), unit(g), unit(b), 0]),
            Color::Hsb { h, s, b } => (1, [unit(h.rem_euclid(360.0) / 360.0), unit(s), unit(b), 0]),
            Color::Cmyk { c, m, y, k } => (
                2,
                [unit(1.0 - c), unit(1.0 - m), unit(1.0 - y), unit(1.0 - k)],
            ),
            Color::Lab { l, a, b } => {
                let signed =
                    |value: f64| ((value * 100.0).round().clamp(-32768.0, 32767.0) as i16) as u16;
                (7, [signed(l), signed(a), signed(b), 0])
            }
            Color::Gray { k } => (8, [(k.clamp(0.0, 1.0) * 10000.0).round() as u16, 0, 0, 0]),
            Color::Other { space, values } => (space, values),
        };
        writer.u16(space);
        for value in values {
            writer.u16(value);
        }
    }

    pub fn from_descriptor(descriptor: &Descriptor) -> Option<Self> {
        let get = |key: &str| descriptor.number(key);
        if descriptor.class.is("RGBC") {
            if let (Some(r), Some(g), Some(b)) = (get("Rd  "), get("Grn "), get("Bl  ")) {
                return Some(Color::Rgb {
                    r: r / 255.0,
                    g: g / 255.0,
                    b: b / 255.0,
                });
            }
            return Some(Color::Rgb {
                r: get("redFloat")?,
                g: get("greenFloat")?,
                b: get("blueFloat")?,
            });
        }
        if descriptor.class.is("HSBC") {
            return Some(Color::Hsb {
                h: get("H   ")?,
                s: get("Strt")? / 100.0,
                b: get("Brgh")? / 100.0,
            });
        }
        if descriptor.class.is("CMYC") {
            return Some(Color::Cmyk {
                c: get("Cyn ")? / 100.0,
                m: get("Mgnt")? / 100.0,
                y: get("Ylw ")? / 100.0,
                k: get("Blck")? / 100.0,
            });
        }
        if descriptor.class.is("LbCl") {
            return Some(Color::Lab {
                l: get("Lmnc")?,
                a: get("A   ")?,
                b: get("B   ")?,
            });
        }
        if descriptor.class.is("Grsc") {
            return Some(Color::Gray {
                k: get("Gry ")? / 100.0,
            });
        }
        if let (Some(r), Some(g), Some(b)) = (get("Rd  "), get("Grn "), get("Bl  ")) {
            return Some(Color::Rgb {
                r: r / 255.0,
                g: g / 255.0,
                b: b / 255.0,
            });
        }
        None
    }

    pub fn to_descriptor(&self) -> Descriptor {
        match *self {
            Color::Rgb { r, g, b } => Descriptor::new("RGBC")
                .with("Rd  ", r * 255.0)
                .with("Grn ", g * 255.0)
                .with("Bl  ", b * 255.0),
            Color::Hsb { h, s, b } => Descriptor::new("HSBC")
                .with("H   ", Value::UnitDouble(Unit::ANGLE, h))
                .with("Strt", s * 100.0)
                .with("Brgh", b * 100.0),
            Color::Cmyk { c, m, y, k } => Descriptor::new("CMYC")
                .with("Cyn ", c * 100.0)
                .with("Mgnt", m * 100.0)
                .with("Ylw ", y * 100.0)
                .with("Blck", k * 100.0),
            Color::Lab { l, a, b } => Descriptor::new("LbCl")
                .with("Lmnc", l)
                .with("A   ", a)
                .with("B   ", b),
            Color::Gray { k } => Descriptor::new("Grsc").with("Gry ", k * 100.0),
            Color::Other { .. } => {
                let [r, g, b] = self.to_rgb();
                Color::Rgb { r, g, b }.to_descriptor()
            }
        }
    }
}
