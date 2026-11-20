use rikka_model_core::Endian;
use rikka_types::srgb;

use crate::file::{Record, Table, f32s, i16s, i32s};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Domain {
    Point,
    Edge,
    Face,
    Corner,
    Tessface,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Data {
    Bool,
    Int8,
    Int16x2,
    Int32,
    Int32x2,
    Float,
    Float2,
    Float3,
    Float4,
    ColorByte,
    ColorFloat,
}

impl Data {
    fn width(self) -> usize {
        match self {
            Self::Bool | Self::Int8 => 1,
            Self::Int16x2 | Self::Int32 | Self::Float | Self::ColorByte => 4,
            Self::Int32x2 | Self::Float2 => 8,
            Self::Float3 => 12,
            Self::Float4 | Self::ColorFloat => 16,
        }
    }
}

pub(crate) mod legacy {
    pub(crate) const MVERT: i64 = 0;
    pub(crate) const MDEFORMVERT: i64 = 2;
    pub(crate) const MEDGE: i64 = 3;
    pub(crate) const MFACE: i64 = 4;
    pub(crate) const MTFACE: i64 = 5;
    pub(crate) const MCOL: i64 = 6;
    pub(crate) const MLOOPUV: i64 = 16;
    pub(crate) const MPOLY: i64 = 25;
    pub(crate) const MLOOP: i64 = 26;
    pub(crate) const BWEIGHT: i64 = 29;
    pub(crate) const CREASE: i64 = 30;
    pub(crate) const CUSTOMLOOPNORMAL: i64 = 41;
    pub(crate) const PROP_FLOAT2: i64 = 49;
}

#[derive(Clone)]
pub(crate) struct Layer<'a> {
    pub(crate) domain: Domain,
    pub(crate) name: String,
    pub(crate) legacy: Option<i64>,
    pub(crate) data: Option<Data>,
    pub(crate) active: bool,
    pub(crate) render: bool,
    pub(crate) flag: i64,
    single: bool,
    holder: Record<'a>,
}

impl<'a> Layer<'a> {
    pub(crate) fn raw(&self) -> Option<&'a [u8]> {
        self.holder.raw("data")
    }

    pub(crate) fn table(&self, layout: &str) -> Option<Table<'a>> {
        self.holder.table("data", layout)
    }

    fn values(&self, data: Data, count: usize) -> Option<&'a [u8]> {
        let bytes = self.raw()?;
        let width = data.width();
        match self.single {
            true => bytes.get(..width),
            false => bytes.get(..count * width),
        }
    }

    fn spread<T: Clone>(&self, values: Vec<T>, count: usize) -> Vec<T> {
        match (self.single, values.first()) {
            (true, Some(first)) => vec![first.clone(); count],
            _ => values,
        }
    }

    pub(crate) fn floats<const N: usize>(
        &self,
        count: usize,
        endian: Endian,
    ) -> Option<Vec<[f32; N]>> {
        let data = self.data?;
        let expected = match N {
            1 => Data::Float,
            2 => Data::Float2,
            3 => Data::Float3,
            _ => Data::Float4,
        };
        if data != expected && !(N == 4 && data == Data::ColorFloat) {
            return None;
        }
        let values = f32s(self.values(data, count)?, endian);
        let rows = values.as_chunks::<N>().0.to_vec();
        Some(self.spread(rows, count))
    }

    pub(crate) fn ints(&self, count: usize, endian: Endian) -> Option<Vec<i32>> {
        match self.data? {
            Data::Int32 => Some(self.spread(i32s(self.values(Data::Int32, count)?, endian), count)),
            Data::Int8 => Some(
                self.spread(
                    self.values(Data::Int8, count)?
                        .iter()
                        .map(|&byte| i32::from(byte as i8))
                        .collect(),
                    count,
                ),
            ),
            _ => None,
        }
    }

    pub(crate) fn int_pairs(&self, count: usize, endian: Endian) -> Option<Vec<[i32; 2]>> {
        match self.data? {
            Data::Int32x2 => {
                let values = i32s(self.values(Data::Int32x2, count)?, endian);
                Some(self.spread(values.as_chunks::<2>().0.to_vec(), count))
            }
            Data::Int16x2 => {
                let values = i16s(self.values(Data::Int16x2, count)?, endian);
                Some(
                    self.spread(
                        values
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|pair| pair.map(i32::from))
                            .collect(),
                        count,
                    ),
                )
            }
            _ => None,
        }
    }

    pub(crate) fn bools(&self, count: usize) -> Option<Vec<bool>> {
        if self.data? != Data::Bool {
            return None;
        }
        let values = self.values(Data::Bool, count)?;
        Some(self.spread(values.iter().map(|&byte| byte != 0).collect(), count))
    }

    pub(crate) fn colors(&self, count: usize, endian: Endian) -> Option<Vec<[f32; 4]>> {
        match self.data? {
            Data::ColorFloat => self.floats::<4>(count, endian),
            Data::ColorByte => {
                let bytes = self.values(Data::ColorByte, count)?;
                let rows = bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|color| {
                        [
                            srgb_to_linear(color[0]),
                            srgb_to_linear(color[1]),
                            srgb_to_linear(color[2]),
                            f32::from(color[3]) / 255.0,
                        ]
                    })
                    .collect();
                Some(self.spread(rows, count))
            }
            _ => None,
        }
    }
}

pub(crate) fn srgb_to_linear(value: u8) -> f32 {
    srgb::to_linear(f32::from(value) / 255.0)
}

pub(crate) struct Attributes<'a> {
    pub(crate) layers: Vec<Layer<'a>>,
}

impl<'a> Attributes<'a> {
    pub(crate) fn of_mesh(mesh: Record<'a>) -> Self {
        let generic = mesh.blend().version() >= 360;
        let mut layers = Vec::new();
        for (field, alias, domain) in [
            ("vdata", "vert_data", Domain::Point),
            ("edata", "edge_data", Domain::Edge),
            ("pdata", "face_data", Domain::Face),
            ("ldata", "corner_data", Domain::Corner),
            ("fdata", "fdata_legacy", Domain::Tessface),
        ] {
            if let Some(data) = mesh.record(field).or_else(|| mesh.record(alias)) {
                custom_data(data, domain, generic, &mut layers);
            }
        }
        if let Some(storage) = mesh.record("attribute_storage") {
            attribute_storage(storage, &mut layers);
        }
        Self { layers }
    }

    pub(crate) fn named(&self, domain: Domain, name: &str) -> Option<&Layer<'a>> {
        self.layers
            .iter()
            .find(|layer| layer.domain == domain && layer.name == name)
    }

    pub(crate) fn legacy(&self, domain: Domain, kind: i64) -> impl Iterator<Item = &Layer<'a>> {
        self.layers
            .iter()
            .filter(move |layer| layer.domain == domain && layer.legacy == Some(kind))
    }

    pub(crate) fn first_legacy(&self, domain: Domain, kind: i64) -> Option<&Layer<'a>> {
        self.legacy(domain, kind).next()
    }

    pub(crate) fn property_name(&self, index: i64) -> Option<&str> {
        let index = usize::try_from(index).ok()?;
        self.layers
            .iter()
            .filter(|layer| layer.domain != Domain::Tessface && layer.data.is_some())
            .nth(index)
            .map(|layer| layer.name.as_str())
    }
}

fn custom_data<'a>(data: Record<'a>, domain: Domain, generic: bool, layers: &mut Vec<Layer<'a>>) {
    let records = data.deref_all("layers");
    let total = data.int("totlayer").unwrap_or(0).max(0) as usize;
    let mut first_of_type: Vec<(i64, usize)> = Vec::new();
    for (index, layer) in records.iter().take(total).enumerate() {
        let kind = layer.int("type").unwrap_or(-1);
        if !first_of_type
            .iter()
            .any(|(first_kind, _)| *first_kind == kind)
        {
            first_of_type.push((kind, index));
        }
    }
    for (index, layer) in records.iter().take(total).enumerate() {
        let kind = layer.int("type").unwrap_or(-1);
        let first = first_of_type
            .iter()
            .find(|(first_kind, _)| *first_kind == kind)
            .map_or(index, |(_, first)| *first);
        let leader = records[first];
        let place = (index - first) as i64;
        layers.push(Layer {
            domain,
            name: layer.text("name").unwrap_or_default(),
            legacy: Some(kind),
            data: custom_data_kind(kind, generic),
            active: leader.int("active") == Some(place),
            render: leader.int("active_rnd") == Some(place),
            flag: layer.int("flag").unwrap_or(0),
            single: false,
            holder: *layer,
        });
    }
}

fn custom_data_kind(kind: i64, generic: bool) -> Option<Data> {
    Some(match kind {
        10 | 29 | 30 => Data::Float,
        11 => Data::Int32,
        17 => Data::ColorByte,
        20 => return None,
        22 if generic => Data::Int16x2,
        24 if generic => Data::Float4,
        41 => Data::Int16x2,
        45 => Data::Int8,
        46 => Data::Int32x2,
        47 => Data::ColorFloat,
        48 => Data::Float3,
        49 => Data::Float2,
        50 => Data::Bool,
        _ => return None,
    })
}

fn attribute_storage<'a>(storage: Record<'a>, layers: &mut Vec<Layer<'a>>) {
    let count = storage.int("dna_attributes_num").unwrap_or(0).max(0) as usize;
    for attribute in storage.deref_all("dna_attributes").into_iter().take(count) {
        let domain = match attribute.int("domain") {
            Some(0) => Domain::Point,
            Some(1) => Domain::Edge,
            Some(2) => Domain::Face,
            Some(3) => Domain::Corner,
            _ => continue,
        };
        let data = match attribute.int("data_type") {
            Some(0) => Data::Bool,
            Some(1) => Data::Int8,
            Some(2) => Data::Int16x2,
            Some(3) => Data::Int32,
            Some(4) => Data::Int32x2,
            Some(5) => Data::Float,
            Some(6) => Data::Float2,
            Some(7) => Data::Float3,
            Some(9) => Data::ColorByte,
            Some(10) => Data::ColorFloat,
            Some(13) => Data::Float4,
            _ => continue,
        };
        let Some(holder) = attribute.deref("data") else {
            continue;
        };
        let single = match attribute.int("storage_type") {
            Some(0) => holder.int("is_single").unwrap_or(0) != 0,
            Some(1) => true,
            _ => continue,
        };
        layers.push(Layer {
            domain,
            name: attribute.string("name").unwrap_or_default(),
            legacy: None,
            data: Some(data),
            active: false,
            render: false,
            flag: 0,
            single,
            holder,
        });
    }
}
