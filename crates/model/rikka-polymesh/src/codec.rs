use glam::{Vec2, Vec3, Vec4};
use rikka_file_format::{Codec, DecodeError, Reader, Writer};

use crate::attributes::{Attribute, Attributes};
use crate::domain::Domain;
use crate::error::Error;
use crate::layers::Layers;
use crate::polymesh::Polymesh;
use crate::shape_keys::{ShapeKey, ShapeKeys};
use crate::values::Values;
use crate::vertex_groups::{Influence, VertexGroup, VertexGroups};

fn put_list<T>(out: &mut Writer<'_>, items: &[T], put: impl Fn(&mut Writer<'_>, &T)) {
    out.count64(items.len());
    for item in items {
        put(out, item);
    }
}

fn list<T>(
    input: &mut Reader<'_>,
    least_bytes: usize,
    mut get: impl FnMut(&mut Reader<'_>) -> Result<T, DecodeError>,
) -> Result<Vec<T>, DecodeError> {
    let count = input.count64(least_bytes)?;
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        items.push(get(input)?);
    }
    Ok(items)
}

fn put_plain<T: Codec>(out: &mut Writer<'_>, items: &[T]) {
    out.count64(items.len());
    out.put_all(items);
}

fn plain<T: Codec<Error = DecodeError>>(
    input: &mut Reader<'_>,
    least_bytes: usize,
) -> Result<Vec<T>, DecodeError> {
    list(input, least_bytes, |input| input.get())
}

fn put_index(out: &mut Writer<'_>, index: Option<usize>) {
    out.put(&index.map(|index| index as u64));
}

fn index(input: &mut Reader<'_>) -> Result<Option<usize>, DecodeError> {
    input
        .get::<Option<u64>>()?
        .map(|index| {
            usize::try_from(index)
                .map_err(|_| DecodeError::Malformed("index exceeds the address space"))
        })
        .transpose()
}

impl Codec for Domain {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.tag(match self {
            Domain::Point => 0,
            Domain::Edge => 1,
            Domain::Face => 2,
            Domain::Corner => 3,
        });
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match input.tag()? {
            0 => Ok(Domain::Point),
            1 => Ok(Domain::Edge),
            2 => Ok(Domain::Face),
            3 => Ok(Domain::Corner),
            _ => Err(DecodeError::Malformed("unknown attribute domain")),
        }
    }
}

impl Codec for Values {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        match self {
            Values::Bool(list) => {
                out.tag(0);
                put_plain(out, list);
            }
            Values::Int(list) => {
                out.tag(1);
                put_plain(out, list);
            }
            Values::Float(list) => {
                out.tag(2);
                put_plain(out, list);
            }
            Values::Vec2(list) => {
                out.tag(3);
                put_plain(out, list);
            }
            Values::Vec3(list) => {
                out.tag(4);
                put_plain(out, list);
            }
            Values::Color(list) => {
                out.tag(5);
                put_plain(out, list);
            }
        }
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match input.tag()? {
            0 => Values::Bool(plain::<bool>(input, 1)?),
            1 => Values::Int(plain::<i32>(input, 4)?),
            2 => Values::Float(plain::<f32>(input, 4)?),
            3 => Values::Vec2(plain::<Vec2>(input, 8)?),
            4 => Values::Vec3(plain::<Vec3>(input, 12)?),
            5 => Values::Color(plain::<Vec4>(input, 16)?),
            _ => return Err(DecodeError::Malformed("unknown attribute data type")),
        })
    }
}

impl Codec for Attribute {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.str(&self.name);
        out.put(&self.domain);
        out.put(&self.values);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Attribute {
            name: input.str()?.to_owned(),
            domain: input.get()?,
            values: input.get()?,
        })
    }
}

impl Codec for Attributes {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        let attributes: Vec<&Attribute> = self.iter().collect();
        put_list(out, &attributes, |out, attribute| out.put(*attribute));
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let mut attributes = Attributes::new();
        for attribute in list(input, 14, |input| input.get::<Attribute>())? {
            if attributes.insert(attribute).is_some() {
                return Err(DecodeError::Malformed("attribute names repeat"));
            }
        }
        Ok(attributes)
    }
}

impl Codec for Layers {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        put_list(out, &self.names, |out, name| out.str(name));
        put_index(out, self.active);
        put_index(out, self.render);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Layers {
            names: list(input, 4, |input| Ok(input.str()?.to_owned()))?,
            active: index(input)?,
            render: index(input)?,
        })
    }
}

impl Codec for VertexGroups {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        put_list(out, &self.groups, |out, group| {
            out.str(&group.name);
            out.put(&group.locked);
        });
        put_list(out, &self.weights, |out, influences| {
            put_list(out, influences, |out, influence| {
                out.put(&influence.group);
                out.put(&influence.weight);
            });
        });
        put_index(out, self.active);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(VertexGroups {
            groups: list(input, 5, |input| {
                Ok(VertexGroup {
                    name: input.str()?.to_owned(),
                    locked: input.get()?,
                })
            })?,
            weights: list(input, 8, |input| {
                list(input, 8, |input| {
                    Ok(Influence {
                        group: input.get()?,
                        weight: input.get()?,
                    })
                })
            })?,
            active: index(input)?,
        })
    }
}

impl Codec for ShapeKeys {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        put_list(out, &self.keys, |out, key| {
            out.str(&key.name);
            put_plain(out, &key.positions);
            out.put(&key.value);
            out.put(&key.min);
            out.put(&key.max);
            out.put(&key.relative_to);
            out.put(&key.vertex_group);
            out.put(&key.muted);
        });
        put_index(out, self.active);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(ShapeKeys {
            keys: list(input, 30, |input| {
                Ok(ShapeKey {
                    name: input.str()?.to_owned(),
                    positions: plain(input, 12)?,
                    value: input.get()?,
                    min: input.get()?,
                    max: input.get()?,
                    relative_to: input.get()?,
                    vertex_group: input.get()?,
                    muted: input.get()?,
                })
            })?,
            active: index(input)?,
        })
    }
}

impl Codec for Polymesh {
    type Error = Error;

    fn encode(&self, out: &mut Writer<'_>) {
        put_plain(out, &self.positions);
        put_plain(out, &self.edges);
        put_plain(out, &self.face_offsets);
        put_plain(out, &self.corner_verts);
        put_plain(out, &self.corner_edges);
        out.put(&self.attributes);
        out.put(&self.uv_maps);
        out.put(&self.color_attributes);
        out.put(&self.vertex_groups);
        out.put(&self.shape_keys);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, Error> {
        let mesh = Polymesh {
            positions: plain(input, 12)?,
            edges: plain(input, 8)?,
            face_offsets: plain(input, 4)?,
            corner_verts: plain(input, 4)?,
            corner_edges: plain(input, 4)?,
            attributes: input.get()?,
            uv_maps: input.get()?,
            color_attributes: input.get()?,
            vertex_groups: input.get()?,
            shape_keys: input.get()?,
        };
        mesh.validate()?;
        Ok(mesh)
    }
}
