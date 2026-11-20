use rikka_annexb::{BitReader, BitWriter, insert_emulation_prevention, strip_emulation_prevention};

const HIGH_PROFILES: [u32; 11] = [100, 110, 122, 244, 44, 83, 86, 118, 128, 138, 144];

const EXTENDED_SAR: u32 = 255;

pub(crate) fn rewrite(nal: &[u8]) -> Option<Vec<u8>> {
    let (&header, escaped) = nal.split_first()?;
    let raw = strip_emulation_prevention(escaped);
    let mut read = BitReader::new(&raw);
    let mut write = BitWriter::default();
    copy_sequence(&mut read, &mut write)?;
    let mut rewritten = vec![header];
    rewritten.extend(insert_emulation_prevention(&write.finish()));
    Some(rewritten)
}

fn copy_sequence(read: &mut BitReader<'_>, write: &mut BitWriter) -> Option<()> {
    let profile = copy(read, write, 8)?;
    copy(read, write, 16)?;
    golomb(read, write)?;
    if HIGH_PROFILES.contains(&profile) {
        let chroma = golomb(read, write)?;
        if chroma == 3 {
            copy(read, write, 1)?;
        }
        golomb(read, write)?;
        golomb(read, write)?;
        copy(read, write, 1)?;
        if copy(read, write, 1)? == 1 {
            let lists = if chroma == 3 { 12 } else { 8 };
            for index in 0..lists {
                if copy(read, write, 1)? == 1 {
                    scaling_list(read, write, if index < 6 { 16 } else { 64 })?;
                }
            }
        }
    }
    golomb(read, write)?;
    match golomb(read, write)? {
        0 => {
            golomb(read, write)?;
        }
        1 => {
            copy(read, write, 1)?;
            signed(read, write)?;
            signed(read, write)?;
            for _ in 0..golomb(read, write)? {
                signed(read, write)?;
            }
        }
        _ => {}
    }
    let references = golomb(read, write)?;
    copy(read, write, 1)?;
    golomb(read, write)?;
    golomb(read, write)?;
    if copy(read, write, 1)? == 0 {
        copy(read, write, 1)?;
    }
    copy(read, write, 1)?;
    if copy(read, write, 1)? == 1 {
        for _ in 0..4 {
            golomb(read, write)?;
        }
    }
    let usable = read.bit()?;
    write.bit(1);
    if usable == 0 {
        write.bits(0, 3);
        write.bits(0, 5);
        write.bit(1);
        restriction(write, references);
        return Some(());
    }
    copy_usability(read, write, references)
}

fn copy_usability(read: &mut BitReader<'_>, write: &mut BitWriter, references: u32) -> Option<()> {
    if copy(read, write, 1)? == 1 && copy(read, write, 8)? == EXTENDED_SAR {
        copy(read, write, 32)?;
    }
    if copy(read, write, 1)? == 1 {
        copy(read, write, 1)?;
    }
    if copy(read, write, 1)? == 1 {
        copy(read, write, 4)?;
        if copy(read, write, 1)? == 1 {
            copy(read, write, 24)?;
        }
    }
    if copy(read, write, 1)? == 1 {
        golomb(read, write)?;
        golomb(read, write)?;
    }
    if copy(read, write, 1)? == 1 {
        copy(read, write, 32)?;
        copy(read, write, 32)?;
        copy(read, write, 1)?;
    }
    let nal_hrd = copy(read, write, 1)?;
    if nal_hrd == 1 {
        hypothetical_decoder(read, write)?;
    }
    let vcl_hrd = copy(read, write, 1)?;
    if vcl_hrd == 1 {
        hypothetical_decoder(read, write)?;
    }
    if nal_hrd == 1 || vcl_hrd == 1 {
        copy(read, write, 1)?;
    }
    copy(read, write, 1)?;
    let restricted = read.bit()?;
    write.bit(1);
    if restricted == 1 {
        copy(read, write, 1)?;
        for _ in 0..4 {
            golomb(read, write)?;
        }
        read.golomb()?;
        read.golomb()?;
        write.golomb(0);
        write.golomb(references);
        return Some(());
    }
    restriction(write, references);
    Some(())
}

fn restriction(write: &mut BitWriter, references: u32) {
    write.bit(1);
    write.golomb(2);
    write.golomb(1);
    write.golomb(16);
    write.golomb(16);
    write.golomb(0);
    write.golomb(references);
}

fn hypothetical_decoder(read: &mut BitReader<'_>, write: &mut BitWriter) -> Option<()> {
    let count = golomb(read, write)?;
    copy(read, write, 8)?;
    for _ in 0..=count {
        golomb(read, write)?;
        golomb(read, write)?;
        copy(read, write, 1)?;
    }
    copy(read, write, 20)?;
    Some(())
}

fn scaling_list(read: &mut BitReader<'_>, write: &mut BitWriter, size: usize) -> Option<()> {
    let mut last = 8;
    let mut next = 8;
    for _ in 0..size {
        if next != 0 {
            let delta = signed(read, write)?;
            next = (last + delta + 256) % 256;
        }
        if next != 0 {
            last = next;
        }
    }
    Some(())
}

fn copy(read: &mut BitReader<'_>, write: &mut BitWriter, count: u32) -> Option<u32> {
    let mut value = 0;
    let mut left = count;
    while left > 0 {
        let step = left.min(16);
        let part = read.bits(step)?;
        write.bits(part, step);
        value = if count > 16 { 0 } else { value << step | part };
        left -= step;
    }
    Some(value)
}

fn golomb(read: &mut BitReader<'_>, write: &mut BitWriter) -> Option<u32> {
    let value = read.golomb()?;
    write.golomb(value);
    Some(value)
}

fn signed(read: &mut BitReader<'_>, write: &mut BitWriter) -> Option<i32> {
    let value = read.signed_golomb()?;
    write.signed_golomb(value);
    Some(value)
}
