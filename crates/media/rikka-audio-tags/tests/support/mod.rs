use std::path::{Path, PathBuf};

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{ItemKey, ItemValue, Tag, TagExt, TagItem, TagType};
use rikka_audio_decode::{WavDepth, WavWriter};

const OPUS_PRE_SKIP: u16 = 312;
const OPUS_RATE: u64 = 48_000;

pub(crate) fn folder() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temporary folder")
}

pub(crate) fn wav(dir: &Path, name: &str, rate: u32, depth: WavDepth, seconds: f32) -> PathBuf {
    let path = dir.join(name);
    let mut writer = WavWriter::create(&path, rate, 2, depth).expect("a WAV writer");
    let frames = (rate as f32 * seconds) as usize;
    writer.write(&vec![0.0; frames * 2]).expect("silence");
    writer.finish().expect("a finished WAV");
    path
}

pub(crate) fn flac(dir: &Path, name: &str, rate: u32, bits: u8, samples: u64) -> PathBuf {
    let mut info = Vec::new();
    info.extend_from_slice(&4096u16.to_be_bytes());
    info.extend_from_slice(&4096u16.to_be_bytes());
    info.extend_from_slice(&[0; 6]);
    let packed = (u64::from(rate) << 44) | (1 << 41) | (u64::from(bits - 1) << 36) | samples;
    info.extend_from_slice(&packed.to_be_bytes());
    info.extend_from_slice(&[0; 16]);
    let mut bytes = b"fLaC".to_vec();
    bytes.push(0x80);
    bytes.extend_from_slice(&u32::try_from(info.len()).unwrap().to_be_bytes()[1..]);
    bytes.extend_from_slice(&info);
    write(dir, name, &bytes)
}

pub(crate) fn mp3(dir: &Path, name: &str, frames: usize) -> PathBuf {
    let mut frame = vec![0u8; 417];
    frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
    write(dir, name, &frame.repeat(frames))
}

pub(crate) fn opus(dir: &Path, name: &str, seconds: u64) -> PathBuf {
    let mut head = b"OpusHead".to_vec();
    head.extend_from_slice(&[1, 2]);
    head.extend_from_slice(&OPUS_PRE_SKIP.to_le_bytes());
    head.extend_from_slice(&44_100u32.to_le_bytes());
    head.extend_from_slice(&[0, 0, 0]);
    let mut tags = b"OpusTags".to_vec();
    tags.extend_from_slice(&5u32.to_le_bytes());
    tags.extend_from_slice(b"rikka");
    tags.extend_from_slice(&0u32.to_le_bytes());
    let end = u64::from(OPUS_PRE_SKIP) + OPUS_RATE * seconds;
    let mut bytes = ogg_page(0x02, 0, 0, &head);
    bytes.extend(ogg_page(0, 0, 1, &tags));
    bytes.extend(ogg_page(0x04, end, 2, &[0xF8, 0xFF, 0xFE]));
    write(dir, name, &bytes)
}

fn ogg_page(kind: u8, granule: u64, sequence: u32, packet: &[u8]) -> Vec<u8> {
    let mut lacing = vec![255u8; packet.len() / 255];
    lacing.push(u8::try_from(packet.len() % 255).unwrap());
    let mut page = b"OggS".to_vec();
    page.extend_from_slice(&[0, kind]);
    page.extend_from_slice(&granule.to_le_bytes());
    page.extend_from_slice(&7u32.to_le_bytes());
    page.extend_from_slice(&sequence.to_le_bytes());
    page.extend_from_slice(&[0; 4]);
    page.push(u8::try_from(lacing.len()).unwrap());
    page.extend_from_slice(&lacing);
    page.extend_from_slice(packet);
    let crc = ogg_crc(&page);
    page[22..26].copy_from_slice(&crc.to_le_bytes());
    page
}

fn ogg_crc(bytes: &[u8]) -> u32 {
    let mut crc = 0u32;
    for byte in bytes {
        crc ^= u32::from(*byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 == 0 {
                crc << 1
            } else {
                (crc << 1) ^ 0x04C1_1DB7
            };
        }
    }
    crc
}

#[derive(Clone, Copy)]
pub(crate) enum Sample {
    Aac,
    Alac { bits: u8, rate: u32 },
}

pub(crate) fn mp4(dir: &Path, name: &str, sample: Sample) -> PathBuf {
    let rate = match sample {
        Sample::Aac => 44_100,
        Sample::Alac { rate, .. } => rate,
    };
    let mut entry = Vec::new();
    entry.extend_from_slice(&[0; 6]);
    entry.extend_from_slice(&1u16.to_be_bytes());
    entry.extend_from_slice(&[0; 8]);
    entry.extend_from_slice(&2u16.to_be_bytes());
    entry.extend_from_slice(&16u16.to_be_bytes());
    entry.extend_from_slice(&[0; 4]);
    let fixed = if rate <= 0xFFFF { rate << 16 } else { 0 };
    entry.extend_from_slice(&fixed.to_be_bytes());
    let entry = match sample {
        Sample::Aac => atom(b"mp4a", &entry),
        Sample::Alac { bits, rate } => {
            let mut config = vec![0; 4];
            config.extend_from_slice(&4096u32.to_be_bytes());
            config.extend_from_slice(&[0, bits, 40, 10, 14, 2]);
            config.extend_from_slice(&255u16.to_be_bytes());
            config.extend_from_slice(&0u32.to_be_bytes());
            config.extend_from_slice(&0u32.to_be_bytes());
            config.extend_from_slice(&rate.to_be_bytes());
            entry.extend(atom(b"alac", &config));
            atom(b"alac", &entry)
        }
    };
    let mut stsd = vec![0; 4];
    stsd.extend_from_slice(&1u32.to_be_bytes());
    stsd.extend(entry);
    let mut mdhd = vec![0; 12];
    mdhd.extend_from_slice(&rate.to_be_bytes());
    mdhd.extend_from_slice(&(rate * 2).to_be_bytes());
    mdhd.extend_from_slice(&[0; 4]);
    let mut hdlr = vec![0; 8];
    hdlr.extend_from_slice(b"soun");
    hdlr.extend_from_slice(&[0; 13]);
    let minf = atom(b"minf", &atom(b"stbl", &atom(b"stsd", &stsd)));
    let mdia = atom(
        b"mdia",
        &[atom(b"mdhd", &mdhd), atom(b"hdlr", &hdlr), minf].concat(),
    );
    let mut ftyp = b"M4A ".to_vec();
    ftyp.extend_from_slice(&[0; 4]);
    ftyp.extend_from_slice(b"M4A isom");
    let bytes = [
        atom(b"ftyp", &ftyp),
        atom(b"moov", &atom(b"trak", &mdia)),
        atom(b"mdat", &[0; 1000]),
    ]
    .concat();
    write(dir, name, &bytes)
}

fn atom(name: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut bytes = u32::try_from(body.len() + 8)
        .unwrap()
        .to_be_bytes()
        .to_vec();
    bytes.extend_from_slice(name);
    bytes.extend_from_slice(body);
    bytes
}

pub(crate) fn dsf(dir: &Path, name: &str, rate: u32, samples: u64, id3: &[u8]) -> PathBuf {
    let data = vec![0x69u8; 4096 * 2];
    let data_at = 28 + 52;
    let metadata = if id3.is_empty() {
        0
    } else {
        data_at + 12 + data.len() as u64
    };
    let total = data_at + 12 + data.len() as u64 + id3.len() as u64;
    let mut bytes = b"DSD ".to_vec();
    bytes.extend_from_slice(&28u64.to_le_bytes());
    bytes.extend_from_slice(&total.to_le_bytes());
    bytes.extend_from_slice(&metadata.to_le_bytes());
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&52u64.to_le_bytes());
    for value in [1u32, 0, 2, 2, rate, 1] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&samples.to_le_bytes());
    bytes.extend_from_slice(&4096u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(12 + data.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&data);
    bytes.extend_from_slice(id3);
    write(dir, name, &bytes)
}

pub(crate) fn dff(dir: &Path, name: &str, rate: u32, sound_bytes: u64, id3: &[u8]) -> PathBuf {
    let mut properties = b"SND ".to_vec();
    properties.extend(chunk(b"FS  ", &rate.to_be_bytes()));
    let mut channels = 2u16.to_be_bytes().to_vec();
    channels.extend_from_slice(b"SLFTSRGT");
    properties.extend(chunk(b"CHNL", &channels));
    properties.extend(chunk(b"CMPR", b"DSD \x0enot compressed\0"));
    let mut body = b"DSD ".to_vec();
    body.extend(chunk(b"FVER", &0x0105_0000u32.to_be_bytes()));
    body.extend(chunk(b"PROP", &properties));
    body.extend(chunk(
        b"DSD ",
        &vec![0x69; usize::try_from(sound_bytes).unwrap()],
    ));
    if !id3.is_empty() {
        body.extend(chunk(b"ID3 ", id3));
    }
    write(dir, name, &chunk(b"FRM8", &body))
}

fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.extend_from_slice(&(body.len() as u64).to_be_bytes());
    bytes.extend_from_slice(body);
    if body.len() % 2 == 1 {
        bytes.push(0);
    }
    bytes
}

pub(crate) fn id3v2(items: &[(ItemKey, &str)]) -> Vec<u8> {
    let tag = tag(TagType::Id3v2, items);
    let mut bytes = Vec::new();
    tag.dump_to(&mut bytes, WriteOptions::default())
        .expect("an ID3v2 tag");
    bytes
}

pub(crate) fn tag(kind: TagType, items: &[(ItemKey, &str)]) -> Tag {
    let mut tag = Tag::new(kind);
    for (key, value) in items {
        tag.push_unchecked(TagItem::new(*key, ItemValue::Text((*value).to_owned())));
    }
    tag
}

pub(crate) fn save(path: &Path, tag: &Tag) {
    tag.save_to_path(path, WriteOptions::default())
        .expect("tags saved");
}

pub(crate) fn picture(kind: PictureType, data: &[u8]) -> Picture {
    Picture::unchecked(data.to_vec())
        .pic_type(kind)
        .mime_type(MimeType::Png)
        .build()
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("a fixture file");
    path
}
