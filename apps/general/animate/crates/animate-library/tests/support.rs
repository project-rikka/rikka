use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use animate_library::{Catalog, Library, Locations, Shelf};

pub(crate) const MIKU: &[u8] =
    include_bytes!("../../../../../../crates/model/rikka-mmd/assets/mmd/miku.pmx");
pub(crate) const DANCE: &[u8] =
    include_bytes!("../../../../../../crates/model/rikka-mmd/assets/mmd/motion.vmd");
pub(crate) const AVATAR: &[u8] =
    include_bytes!("../../../../../../crates/model/rikka-vrm/assets/avatar.vrm");

const PATIENCE: Duration = Duration::from_secs(30);

pub(crate) struct Opened {
    pub(crate) library: Library,
    catalogs: Receiver<Arc<Catalog>>,
}

impl Opened {
    pub(crate) fn new(locations: Locations) -> Opened {
        let (sender, catalogs) = mpsc::channel();
        let library = Library::open(locations, move |catalog| {
            sender.send(catalog).expect("test stopped listening");
        })
        .expect("library opens");
        Opened { library, catalogs }
    }

    pub(crate) fn until(&self, mut settled: impl FnMut(&Catalog) -> bool) -> Arc<Catalog> {
        loop {
            let catalog = self
                .catalogs
                .recv_timeout(PATIENCE)
                .expect("library published nothing in time");
            if settled(&catalog) {
                return catalog;
            }
        }
    }

    pub(crate) fn probed(&self) -> Arc<Catalog> {
        self.until(|catalog| catalog.pending() == 0)
    }
}

pub(crate) fn names(catalog: &Catalog, shelf: Shelf) -> Vec<String> {
    catalog
        .shelf(shelf)
        .entries()
        .iter()
        .map(|entry| entry.name().to_string())
        .collect()
}

pub(crate) fn put(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("file has a folder")).expect("folder created");
    std::fs::write(&path, bytes).expect("file written");
}

pub(crate) fn vmd(bones: u32, morphs: u32, cameras: u32, last_frame: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut header = b"Vocaloid Motion Data 0002".to_vec();
    header.resize(30, 0);
    bytes.extend_from_slice(&header);
    let mut name = b"model".to_vec();
    name.resize(20, 0);
    bytes.extend_from_slice(&name);
    section(&mut bytes, bones, 111, 15, last_frame);
    section(&mut bytes, morphs, 23, 15, last_frame);
    section(&mut bytes, cameras, 61, 0, last_frame);
    bytes.extend_from_slice(&[0; 12]);
    bytes
}

fn section(bytes: &mut Vec<u8>, count: u32, record: usize, frame: usize, last_frame: u32) {
    bytes.extend_from_slice(&count.to_le_bytes());
    for index in 0..count {
        let mut fields = vec![0; record];
        let at = match index + 1 == count {
            true => last_frame,
            false => index,
        };
        fields[frame..frame + 4].copy_from_slice(&at.to_le_bytes());
        bytes.extend_from_slice(&fields);
    }
}

pub(crate) fn glb(json: &str) -> Vec<u8> {
    let mut chunk = json.as_bytes().to_vec();
    while !chunk.len().is_multiple_of(4) {
        chunk.push(b' ');
    }
    let length = 12 + 8 + chunk.len();
    let mut bytes = b"glTF".to_vec();
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(length).expect("small").to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(chunk.len()).expect("small").to_le_bytes());
    bytes.extend_from_slice(b"JSON");
    bytes.extend_from_slice(&chunk);
    bytes
}

pub(crate) const CLIP_ONLY: &str = r#"{"asset":{"version":"2.0"},"accessors":[{"count":2,"max":[1.5],"min":[0.0],"type":"SCALAR","componentType":5126}],"animations":[{"name":"wave","channels":[],"samplers":[{"input":0,"output":0}]}]}"#;
pub(crate) const BARE: &str = r#"{"asset":{"version":"2.0"}}"#;

pub(crate) fn wav(samples: u32) -> Vec<u8> {
    let rate: u32 = 8000;
    let data = samples * 2;
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    bytes.resize(bytes.len() + data as usize, 0);
    bytes
}

pub(crate) fn stl(triangles: u32) -> Vec<u8> {
    let mut bytes = vec![0; 80];
    bytes.extend_from_slice(&triangles.to_le_bytes());
    bytes.resize(84 + 50 * triangles as usize, 0);
    bytes
}
