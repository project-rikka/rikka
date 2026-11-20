use rikka_renderer_3d::glam::Vec3;
use rikka_shader_graph::{Constant, Datum, Record};

use crate::asset::Asset;
use crate::error::Error;
use crate::name::Name;
use crate::world::choice::Choice;

pub(crate) struct Read<'r> {
    record: &'r Record,
}

impl<'r> Read<'r> {
    pub(crate) fn new(record: &'r Record) -> Self {
        Self { record }
    }

    fn missing(&self, port: &str, expected: &'static str) -> Error {
        Error::Record {
            node: self.record.id.clone(),
            port: port.to_owned(),
            expected,
        }
    }

    pub(crate) fn number(&self, port: &str) -> Result<Constant, Error> {
        self.record
            .number(port)
            .ok_or_else(|| self.missing(port, "a number"))
    }

    pub(crate) fn float(&self, port: &str) -> Result<f32, Error> {
        Ok(self.number(port)?.component(0))
    }

    pub(crate) fn vec3(&self, port: &str) -> Result<Vec3, Error> {
        let value = self.number(port)?.resized(3);
        Ok(Vec3::new(
            value.component(0),
            value.component(1),
            value.component(2),
        ))
    }

    pub(crate) fn toggle(&self, port: &str) -> Result<bool, Error> {
        Ok(self.float(port)? > 0.5)
    }

    pub(crate) fn whole(&self, port: &str) -> Result<u32, Error> {
        let value = self.float(port)?;
        Ok(match value.is_finite() {
            true => value.round().max(0.0) as u32,
            false => 0,
        })
    }

    pub(crate) fn choice<T: Choice>(&self, port: &str) -> Result<T, Error> {
        Ok(T::picked(self.float(port)?))
    }

    pub(crate) fn text(&self, port: &str) -> Result<Option<&'r str>, Error> {
        match self.record.field(port) {
            Some(Datum::Asset(held)) => Ok(held
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty())),
            _ => Err(self.missing(port, "a file or text")),
        }
    }

    pub(crate) fn asset(&self, port: &str) -> Result<Option<Asset>, Error> {
        Ok(self.text(port)?.map(Asset::new))
    }

    pub(crate) fn name(&self, port: &str) -> Result<Option<Name>, Error> {
        Ok(self.text(port)?.map(Name::new))
    }

    pub(crate) fn names(&self, port: &str) -> Result<Vec<Name>, Error> {
        Ok(self
            .text(port)?
            .map(|list| {
                list.split(',')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(Name::new)
                    .collect()
            })
            .unwrap_or_default())
    }
}
