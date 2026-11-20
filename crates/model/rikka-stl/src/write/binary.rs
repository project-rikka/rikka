use rikka_model_core::{ByteWriter, Error, Requirement, Result};

use crate::facet::{Solid, viscam};
use crate::read::binary::{FACET, HEADER, PREAMBLE};

const SIGNATURE: &[u8] = b"Binary STL written by Rikka";

pub(crate) fn write(solids: &[Solid]) -> Result<Vec<u8>> {
    let count: usize = solids.iter().map(|solid| solid.facets.len()).sum();
    let declared = u32::try_from(count).map_err(|_| Error::Unmet {
        format: "binary STL",
        requirement: Requirement::Triangles {
            actual: count as u64,
            allowed: u64::from(u32::MAX),
        },
    })?;
    let mut writer = ByteWriter::with_capacity(PREAMBLE + count * FACET);
    writer.fixed(SIGNATURE, HEADER);
    writer.u32(declared);
    for facet in solids.iter().flat_map(|solid| &solid.facets) {
        writer.vec3(facet.normal);
        for corner in facet.corners {
            writer.vec3(corner);
        }
        writer.u16(facet.color.map_or(0, viscam));
    }
    Ok(writer.finish())
}
