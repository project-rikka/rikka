use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::details::{Details, ModelDetails};

use super::fault::{Fault, Probed, text};

const HEADER: u64 = 84;
const COUNT: usize = 80;
const TRIANGLE: u64 = 50;
const ASCII: &[u8] = b"solid";

pub(super) fn read(path: &Path) -> Probed<Details> {
    let file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut head = Vec::new();
    file.take(HEADER).read_to_end(&mut head)?;
    if let Some(count) = head.get(COUNT..COUNT + 4) {
        let triangles = u64::from(u32::from_le_bytes([count[0], count[1], count[2], count[3]]));
        if HEADER + triangles * TRIANGLE == size {
            return Ok(Details::Model(Box::new(ModelDetails {
                vertices: Some(triangles * 3),
                meshes: Some(u32::from(triangles > 0)),
                ..ModelDetails::default()
            })));
        }
    }
    if head.starts_with(ASCII) {
        let title = head
            .split(|byte| *byte == b'\n')
            .next()
            .and_then(|line| text(&line[ASCII.len()..]));
        return Ok(Details::Model(Box::new(ModelDetails {
            title,
            ..ModelDetails::default()
        })));
    }
    Err(Fault::Foreign("STL"))
}
