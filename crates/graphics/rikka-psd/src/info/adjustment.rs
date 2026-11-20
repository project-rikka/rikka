use crate::color::Color;
use crate::error::{Error, Result};
use crate::io::{Reader, Writer};
use crate::key::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelRecord {
    pub input_floor: u16,
    pub input_ceiling: u16,
    pub output_floor: u16,
    pub output_ceiling: u16,
    pub gamma: u16,
}

impl Default for LevelRecord {
    fn default() -> Self {
        Self {
            input_floor: 0,
            input_ceiling: 255,
            output_floor: 0,
            output_ceiling: 255,
            gamma: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Levels {
    pub version: u16,
    pub records: Vec<LevelRecord>,
    pub extra_version: Option<u16>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CurveData {
    Points(Vec<(u16, u16)>),
    Map(Box<[u8; 256]>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Curves {
    pub is_map: bool,
    pub version: u16,
    pub channel_mask: u32,
    pub curves: Vec<CurveData>,
    pub extra: Option<(u16, Vec<(u16, CurveData)>)>,
}

impl Curves {
    pub fn channel_curves(&self) -> Vec<(u16, &CurveData)> {
        if let Some((_, items)) = &self.extra {
            return items
                .iter()
                .map(|(channel, data)| (*channel, data))
                .collect();
        }
        if self.version == 1 {
            let mut channels = (0..32u16).filter(|bit| self.channel_mask & (1 << bit) != 0);
            self.curves
                .iter()
                .filter_map(|data| channels.next().map(|channel| (channel, data)))
                .collect()
        } else {
            self.curves
                .iter()
                .enumerate()
                .map(|(index, data)| (index as u16, data))
                .collect()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhotoFilterColor {
    Lab([i32; 3]),
    Color(Color),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhotoFilter {
    pub version: u16,
    pub color: PhotoFilterColor,
    pub density: u32,
    pub preserve_luminosity: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientMapColorStop {
    pub location: u32,
    pub midpoint: u32,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradientMapOpacityStop {
    pub location: u32,
    pub midpoint: u32,
    pub opacity: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GradientMap {
    pub version: u16,
    pub reversed: bool,
    pub dithered: bool,
    pub method: Key,
    pub name: String,
    pub color_stops: Vec<GradientMapColorStop>,
    pub opacity_stops: Vec<GradientMapOpacityStop>,
    pub expansion: u16,
    pub interpolation: u16,
    pub length: u16,
    pub mode: u16,
    pub random_seed: u32,
    pub show_transparency: u16,
    pub use_vector_color: u16,
    pub roughness: u32,
    pub color_model: u16,
    pub minimum: [u16; 4],
    pub maximum: [u16; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct HueSaturation {
    pub legacy_key: bool,
    pub version: u16,
    pub colorize: bool,
    pub colorization: [i16; 3],
    pub master: [i16; 3],
    pub ranges: [([i16; 4], [i16; 3]); 6],
    pub extra: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Adjustment {
    BrightnessContrast {
        brightness: i16,
        contrast: i16,
        mean: i16,
        lab_only: bool,
    },
    Levels(Levels),
    Curves(Curves),
    Exposure {
        version: u16,
        exposure: f32,
        offset: f32,
        gamma: f32,
    },
    HueSaturation(HueSaturation),
    ColorBalance {
        shadows: [i16; 3],
        midtones: [i16; 3],
        highlights: [i16; 3],
        preserve_luminosity: bool,
    },
    PhotoFilter(PhotoFilter),
    ChannelMixer {
        version: u16,
        monochrome: bool,
        records: Vec<[i16; 5]>,
    },
    Invert,
    Posterize {
        levels: u16,
    },
    Threshold {
        level: u16,
    },
    GradientMap(GradientMap),
    SelectiveColor {
        version: u16,
        absolute: bool,
        plates: [[i16; 4]; 10],
    },
}

pub(crate) fn is_adjustment_key(key: Key) -> bool {
    matches!(
        &key.0,
        b"brit"
            | b"levl"
            | b"curv"
            | b"expA"
            | b"hue2"
            | b"hue "
            | b"blnc"
            | b"phfl"
            | b"mixr"
            | b"nvrt"
            | b"post"
            | b"thrs"
            | b"grdm"
            | b"selc"
    )
}

fn triple(reader: &mut Reader<'_>) -> Result<[i16; 3]> {
    Ok([reader.i16()?, reader.i16()?, reader.i16()?])
}

fn curve(reader: &mut Reader<'_>, is_map: bool) -> Result<CurveData> {
    if is_map {
        let mut map = Box::new([0u8; 256]);
        map.copy_from_slice(reader.bytes(256)?);
        return Ok(CurveData::Map(map));
    }
    let count = reader.u16()?;
    if count > 1024 {
        return Err(reader.malformed("curve points"));
    }
    let mut points = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        points.push((reader.u16()?, reader.u16()?));
    }
    Ok(CurveData::Points(points))
}

fn write_curve(writer: &mut Writer, data: &CurveData) {
    match data {
        CurveData::Map(map) => writer.bytes(map.as_slice()),
        CurveData::Points(points) => {
            writer.u16(points.len() as u16);
            for &(output, input) in points {
                writer.u16(output);
                writer.u16(input);
            }
        }
    }
}

impl Adjustment {
    pub fn key(&self) -> Key {
        Key(*match self {
            Adjustment::BrightnessContrast { .. } => b"brit",
            Adjustment::Levels(_) => b"levl",
            Adjustment::Curves(_) => b"curv",
            Adjustment::Exposure { .. } => b"expA",
            Adjustment::HueSaturation(hue) if hue.legacy_key => b"hue ",
            Adjustment::HueSaturation(_) => b"hue2",
            Adjustment::ColorBalance { .. } => b"blnc",
            Adjustment::PhotoFilter(_) => b"phfl",
            Adjustment::ChannelMixer { .. } => b"mixr",
            Adjustment::Invert => b"nvrt",
            Adjustment::Posterize { .. } => b"post",
            Adjustment::Threshold { .. } => b"thrs",
            Adjustment::GradientMap(_) => b"grdm",
            Adjustment::SelectiveColor { .. } => b"selc",
        })
    }

    pub(crate) fn read(key: Key, reader: &mut Reader<'_>) -> Result<Self> {
        Ok(match &key.0 {
            b"brit" => {
                let brightness = reader.i16()?;
                let contrast = reader.i16()?;
                let mean = reader.i16()?;
                let lab_only = reader.bool()?;
                Adjustment::BrightnessContrast {
                    brightness,
                    contrast,
                    mean,
                    lab_only,
                }
            }
            b"levl" => {
                let version = reader.u16()?;
                let read_record = |reader: &mut Reader<'_>| -> Result<LevelRecord> {
                    Ok(LevelRecord {
                        input_floor: reader.u16()?,
                        input_ceiling: reader.u16()?,
                        output_floor: reader.u16()?,
                        output_ceiling: reader.u16()?,
                        gamma: reader.u16()?,
                    })
                };
                let mut records = Vec::with_capacity(29);
                for _ in 0..29 {
                    records.push(read_record(reader)?);
                }
                let mut extra_version = None;
                if reader.remaining() >= 8 && reader.peek_key() == Some(Key(*b"Lvls")) {
                    reader.skip(4)?;
                    let version = reader.u16()?;
                    let count = reader.u16()?;
                    for _ in 29..count {
                        records.push(read_record(reader)?);
                    }
                    extra_version = Some(version);
                }
                Adjustment::Levels(Levels {
                    version,
                    records,
                    extra_version,
                })
            }
            b"curv" => {
                let is_map = reader.bool()?;
                let version = reader.u16()?;
                let channel_mask = reader.u32()?;
                let count = if version == 1 {
                    channel_mask.count_ones()
                } else {
                    channel_mask
                };
                if count > 64 {
                    return Err(reader.malformed("curves"));
                }
                let mut curves = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    curves.push(curve(reader, is_map)?);
                }
                let mut extra = None;
                if reader.remaining() >= 10 && reader.peek_key() == Some(Key(*b"Crv ")) {
                    reader.skip(4)?;
                    let extra_version = reader.u16()?;
                    let count = reader.u32()?;
                    if count > 64 {
                        return Err(reader.malformed("curves"));
                    }
                    let mut items = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        let channel = reader.u16()?;
                        items.push((channel, curve(reader, is_map)?));
                    }
                    extra = Some((extra_version, items));
                }
                Adjustment::Curves(Curves {
                    is_map,
                    version,
                    channel_mask,
                    curves,
                    extra,
                })
            }
            b"expA" => Adjustment::Exposure {
                version: reader.u16()?,
                exposure: reader.f32()?,
                offset: reader.f32()?,
                gamma: reader.f32()?,
            },
            b"hue2" | b"hue " => {
                let version = reader.u16()?;
                let colorize = reader.bool()?;
                reader.skip(1)?;
                let colorization = triple(reader)?;
                let master = triple(reader)?;
                let mut ranges = [([0i16; 4], [0i16; 3]); 6];
                for range in &mut ranges {
                    for value in &mut range.0 {
                        *value = reader.i16()?;
                    }
                    range.1 = triple(reader)?;
                }
                Adjustment::HueSaturation(HueSaturation {
                    legacy_key: &key.0 == b"hue ",
                    version,
                    colorize,
                    colorization,
                    master,
                    ranges,
                    extra: reader.take_rest().to_vec(),
                })
            }
            b"blnc" => Adjustment::ColorBalance {
                shadows: triple(reader)?,
                midtones: triple(reader)?,
                highlights: triple(reader)?,
                preserve_luminosity: reader.bool()?,
            },
            b"phfl" => {
                let version = reader.u16()?;
                let color = match version {
                    3 => PhotoFilterColor::Lab([reader.i32()?, reader.i32()?, reader.i32()?]),
                    2 => PhotoFilterColor::Color(Color::read(reader)?),
                    other => {
                        return Err(Error::Invalid {
                            what: "photo filter version",
                            value: i64::from(other),
                        });
                    }
                };
                Adjustment::PhotoFilter(PhotoFilter {
                    version,
                    color,
                    density: reader.u32()?,
                    preserve_luminosity: reader.bool()?,
                })
            }
            b"mixr" => {
                let version = reader.u16()?;
                let monochrome = reader.u16()? != 0;
                let mut records = Vec::new();
                while reader.remaining() >= 10 {
                    records.push([
                        reader.i16()?,
                        reader.i16()?,
                        reader.i16()?,
                        reader.i16()?,
                        reader.i16()?,
                    ]);
                }
                Adjustment::ChannelMixer {
                    version,
                    monochrome,
                    records,
                }
            }
            b"nvrt" => Adjustment::Invert,
            b"post" => Adjustment::Posterize {
                levels: reader.u16()?,
            },
            b"thrs" => Adjustment::Threshold {
                level: reader.u16()?,
            },
            b"grdm" => {
                let version = reader.u16()?;
                let reversed = reader.bool()?;
                let dithered = reader.bool()?;
                let method = if version == 3 {
                    reader.key()?
                } else {
                    Key(*b"Gcls")
                };
                let name = reader.unicode()?;
                let count = reader.u16()?;
                let mut color_stops = Vec::with_capacity(usize::from(count));
                for _ in 0..count {
                    let location = reader.u32()?;
                    let midpoint = reader.u32()?;
                    let color = Color::read(reader)?;
                    reader.skip(2)?;
                    color_stops.push(GradientMapColorStop {
                        location,
                        midpoint,
                        color,
                    });
                }
                let count = reader.u16()?;
                let mut opacity_stops = Vec::with_capacity(usize::from(count));
                for _ in 0..count {
                    opacity_stops.push(GradientMapOpacityStop {
                        location: reader.u32()?,
                        midpoint: reader.u32()?,
                        opacity: reader.u16()?,
                    });
                }
                let expansion = reader.u16()?;
                let interpolation = reader.u16()?;
                let length = reader.u16()?;
                let mode = reader.u16()?;
                let random_seed = reader.u32()?;
                let show_transparency = reader.u16()?;
                let use_vector_color = reader.u16()?;
                let roughness = reader.u32()?;
                let color_model = reader.u16()?;
                let mut minimum = [0u16; 4];
                let mut maximum = [0u16; 4];
                for value in &mut minimum {
                    *value = reader.u16()?;
                }
                for value in &mut maximum {
                    *value = reader.u16()?;
                }
                reader.skip(2.min(reader.remaining()))?;
                Adjustment::GradientMap(GradientMap {
                    version,
                    reversed,
                    dithered,
                    method,
                    name,
                    color_stops,
                    opacity_stops,
                    expansion,
                    interpolation,
                    length,
                    mode,
                    random_seed,
                    show_transparency,
                    use_vector_color,
                    roughness,
                    color_model,
                    minimum,
                    maximum,
                })
            }
            b"selc" => {
                let version = reader.u16()?;
                let absolute = reader.u16()? != 0;
                let mut plates = [[0i16; 4]; 10];
                for plate in &mut plates {
                    for value in plate.iter_mut() {
                        *value = reader.i16()?;
                    }
                }
                Adjustment::SelectiveColor {
                    version,
                    absolute,
                    plates,
                }
            }
            _ => return Err(reader.malformed("adjustment")),
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) {
        match self {
            Adjustment::BrightnessContrast {
                brightness,
                contrast,
                mean,
                lab_only,
            } => {
                writer.i16(*brightness);
                writer.i16(*contrast);
                writer.i16(*mean);
                writer.bool(*lab_only);
                writer.u8(0);
            }
            Adjustment::Levels(levels) => {
                writer.u16(levels.version);
                let default = LevelRecord::default();
                let record = |writer: &mut Writer, record: &LevelRecord| {
                    writer.u16(record.input_floor);
                    writer.u16(record.input_ceiling);
                    writer.u16(record.output_floor);
                    writer.u16(record.output_ceiling);
                    writer.u16(record.gamma);
                };
                for index in 0..29 {
                    record(writer, levels.records.get(index).unwrap_or(&default));
                }
                if let Some(version) = levels.extra_version {
                    writer.code(b"Lvls");
                    writer.u16(version);
                    writer.u16(levels.records.len().max(29) as u16);
                    for item in levels.records.iter().skip(29) {
                        record(writer, item);
                    }
                }
            }
            Adjustment::Curves(curves) => {
                writer.bool(curves.is_map);
                writer.u16(curves.version);
                writer.u32(curves.channel_mask);
                for data in &curves.curves {
                    write_curve(writer, data);
                }
                if let Some((version, items)) = &curves.extra {
                    writer.code(b"Crv ");
                    writer.u16(*version);
                    writer.u32(items.len() as u32);
                    for (channel, data) in items {
                        writer.u16(*channel);
                        write_curve(writer, data);
                    }
                }
            }
            Adjustment::Exposure {
                version,
                exposure,
                offset,
                gamma,
            } => {
                writer.u16(*version);
                writer.f32(*exposure);
                writer.f32(*offset);
                writer.f32(*gamma);
            }
            Adjustment::HueSaturation(hue) => {
                writer.u16(hue.version);
                writer.bool(hue.colorize);
                writer.u8(0);
                hue.colorization.iter().for_each(|&value| writer.i16(value));
                hue.master.iter().for_each(|&value| writer.i16(value));
                for (range, settings) in &hue.ranges {
                    range.iter().for_each(|&value| writer.i16(value));
                    settings.iter().for_each(|&value| writer.i16(value));
                }
                writer.bytes(&hue.extra);
            }
            Adjustment::ColorBalance {
                shadows,
                midtones,
                highlights,
                preserve_luminosity,
            } => {
                for values in [shadows, midtones, highlights] {
                    values.iter().for_each(|&value| writer.i16(value));
                }
                writer.bool(*preserve_luminosity);
                writer.u8(0);
            }
            Adjustment::PhotoFilter(filter) => {
                writer.u16(filter.version);
                match filter.color {
                    PhotoFilterColor::Lab(values) => {
                        values.iter().for_each(|&value| writer.i32(value))
                    }
                    PhotoFilterColor::Color(color) => color.write(writer),
                }
                writer.u32(filter.density);
                writer.bool(filter.preserve_luminosity);
            }
            Adjustment::ChannelMixer {
                version,
                monochrome,
                records,
            } => {
                writer.u16(*version);
                writer.u16(u16::from(*monochrome));
                for record in records {
                    record.iter().for_each(|&value| writer.i16(value));
                }
            }
            Adjustment::Invert => {}
            Adjustment::Posterize { levels } => {
                writer.u16(*levels);
                writer.u16(0);
            }
            Adjustment::Threshold { level } => {
                writer.u16(*level);
                writer.u16(0);
            }
            Adjustment::GradientMap(map) => {
                writer.u16(map.version);
                writer.bool(map.reversed);
                writer.bool(map.dithered);
                if map.version == 3 {
                    writer.key(map.method);
                }
                writer.unicode_terminated(&map.name);
                writer.u16(map.color_stops.len() as u16);
                for stop in &map.color_stops {
                    writer.u32(stop.location);
                    writer.u32(stop.midpoint);
                    stop.color.write(writer);
                    writer.u16(0);
                }
                writer.u16(map.opacity_stops.len() as u16);
                for stop in &map.opacity_stops {
                    writer.u32(stop.location);
                    writer.u32(stop.midpoint);
                    writer.u16(stop.opacity);
                }
                writer.u16(map.expansion);
                writer.u16(map.interpolation);
                writer.u16(map.length);
                writer.u16(map.mode);
                writer.u32(map.random_seed);
                writer.u16(map.show_transparency);
                writer.u16(map.use_vector_color);
                writer.u32(map.roughness);
                writer.u16(map.color_model);
                map.minimum.iter().for_each(|&value| writer.u16(value));
                map.maximum.iter().for_each(|&value| writer.u16(value));
                writer.u16(0);
            }
            Adjustment::SelectiveColor {
                version,
                absolute,
                plates,
            } => {
                writer.u16(*version);
                writer.u16(u16::from(*absolute));
                for plate in plates {
                    plate.iter().for_each(|&value| writer.i16(value));
                }
            }
        }
    }
}
