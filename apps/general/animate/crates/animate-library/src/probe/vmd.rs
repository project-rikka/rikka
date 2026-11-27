use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use rikka_encoding::SHIFT_JIS;
use rikka_model::mmd::{CAMERA_MOTION, FRAMES_PER_SECOND};

use crate::details::{Details, MotionDetails};

use super::fault::{Fault, Probed, clean};

const HEADER: usize = 30;
const SIGNATURE: &[u8] = b"Vocaloid Motion Data ";
const CURRENT: &[u8] = b"0002";
const NAME: usize = 20;
const LEGACY_NAME: usize = 10;
const BONE: usize = 111;
const BONE_FRAME: usize = 15;
const MORPH: usize = 23;
const MORPH_FRAME: usize = 15;
const CAMERA: usize = 61;
const CAMERA_FRAME: usize = 0;
const BUFFER: usize = 1 << 16;

pub(super) fn read(path: &Path) -> Probed<Details> {
    let file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut input = Sections {
        input: BufReader::with_capacity(BUFFER, file),
        remaining: size,
        last_frame: 0,
    };
    let header = input.bytes(HEADER)?;
    if !header.starts_with(SIGNATURE) {
        return Err(Fault::Foreign("VMD"));
    }
    let name_length = match header[SIGNATURE.len()..].starts_with(CURRENT) {
        true => NAME,
        false => LEGACY_NAME,
    };
    let name = input.bytes(name_length)?;
    let target = model_name(&name);
    let bones = input.section(BONE, BONE_FRAME)?;
    let morphs = input.section(MORPH, MORPH_FRAME)?;
    let cameras = input.section(CAMERA, CAMERA_FRAME)?;
    let frames = input.last_frame;
    Ok(Details::Motion(Box::new(MotionDetails {
        seconds: frames as f32 / FRAMES_PER_SECOND,
        frames: Some(frames),
        body: bones > 0,
        morphs: morphs > 0,
        camera: cameras > 0,
        target: target.filter(|name| name != CAMERA_MOTION),
        ..MotionDetails::default()
    })))
}

struct Sections {
    input: BufReader<File>,
    remaining: u64,
    last_frame: u32,
}

impl Sections {
    fn bytes(&mut self, length: usize) -> Probed<Vec<u8>> {
        if (length as u64) > self.remaining {
            return Err(Fault::Truncated);
        }
        let mut bytes = vec![0; length];
        self.input.read_exact(&mut bytes).map_err(Fault::read)?;
        self.remaining -= length as u64;
        Ok(bytes)
    }

    fn section(&mut self, record: usize, frame: usize) -> Probed<u32> {
        if self.remaining == 0 {
            return Ok(0);
        }
        let count = u32::from_le_bytes(self.array()?);
        if u64::from(count) * record as u64 > self.remaining {
            return Err(Fault::Truncated);
        }
        let mut bytes = vec![0; record];
        for _ in 0..count {
            self.input.read_exact(&mut bytes).map_err(Fault::read)?;
            let field = [
                bytes[frame],
                bytes[frame + 1],
                bytes[frame + 2],
                bytes[frame + 3],
            ];
            self.last_frame = self.last_frame.max(u32::from_le_bytes(field));
        }
        self.remaining -= u64::from(count) * record as u64;
        Ok(count)
    }

    fn array(&mut self) -> Probed<[u8; 4]> {
        let bytes = self.bytes(4)?;
        Ok([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
}

fn model_name(bytes: &[u8]) -> Option<String> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    let (text, _, _) = SHIFT_JIS.decode(&bytes[..end]);
    clean(&text)
}
