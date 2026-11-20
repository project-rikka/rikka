use std::sync::Arc;

use crate::error::ParseError;
use crate::{cube, three_dl};

pub(crate) const LARGEST: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Domain {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Default for Domain {
    fn default() -> Self {
        Self {
            min: [0.0; 3],
            max: [1.0; 3],
        }
    }
}

#[derive(Clone, Debug)]
pub struct ColorLookup {
    size: usize,
    domain: Domain,
    table: Arc<[f32]>,
}

impl ColorLookup {
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        match cube::is_cube(text) {
            true => cube::parse(text),
            false => three_dl::parse(text),
        }
    }

    pub(crate) fn new(size: usize, domain: Domain, table: Vec<f32>) -> Result<Self, ParseError> {
        if !(2..=LARGEST).contains(&size) {
            return Err(ParseError::Size(size));
        }
        let expected = size * size * size;
        if table.len() != expected * 3 {
            return Err(ParseError::Entries {
                size,
                expected,
                found: table.len() / 3,
            });
        }
        if (0..3).any(|channel| domain.max[channel] <= domain.min[channel]) {
            return Err(ParseError::Domain);
        }
        Ok(Self {
            size,
            domain,
            table: table.into(),
        })
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn domain(&self) -> Domain {
        self.domain
    }

    pub fn table(&self) -> &[f32] {
        &self.table
    }

    pub fn entry(&self, red: usize, green: usize, blue: usize) -> [f32; 3] {
        let at = ((blue * self.size + green) * self.size + red) * 3;
        [self.table[at], self.table[at + 1], self.table[at + 2]]
    }
}

impl PartialEq for ColorLookup {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
            && self.domain == other.domain
            && (Arc::ptr_eq(&self.table, &other.table) || self.table == other.table)
    }
}
