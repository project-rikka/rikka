use crate::atoms::{Atom, Atoms, top_level};
use crate::bytes::Bytes;
use crate::error::{Error, Result};
use crate::fetch::Fetch;
use crate::format::Format;
use crate::heif::meta::Nclx;
use crate::heif::still::Coding;

const MOST_MOVIE_BYTES: u64 = 64 << 20;
const VISUAL_ENTRY_BYTES: usize = 78;
const PICTURES: [u8; 4] = *b"pict";
const AUXILIARY: [u8; 4] = *b"auxv";
const REPEATS: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Sample {
    pub(crate) offset: u64,
    pub(crate) size: u64,
    pub(crate) duration: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Track {
    pub(crate) id: u32,
    pub(crate) coding: Coding,
    pub(crate) nclx: Option<Nclx>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) timescale: u32,
    pub(crate) samples: Vec<Sample>,
    handler: [u8; 4],
    auxiliary_of: Vec<u32>,
    edit: Option<Edit>,
}

#[derive(Clone, Copy, Debug)]
struct Edit {
    repeats: bool,
    segment: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct Sequence {
    pub(crate) color: Track,
    pub(crate) alpha: Option<Track>,
    pub(crate) plays: Option<u32>,
}

impl Sequence {
    pub(crate) fn read(fetch: &mut Fetch<'_>) -> Result<Option<Sequence>> {
        let format = fetch.format();
        let Some(span) = top_level(fetch)?
            .into_iter()
            .find(|span| span.kind == *b"moov")
        else {
            return Ok(None);
        };
        if span.len > MOST_MOVIE_BYTES {
            return Err(Error::unsupported(
                format,
                "movie headers larger than 64 MiB",
            ));
        }
        let file = fetch.len();
        let body = fetch.fetch(span.at, span.len)?;
        Sequence::parse(format, &body, file).map(Some)
    }

    fn parse(format: Format, movie: &[u8], file: u64) -> Result<Sequence> {
        let mut duration = None;
        let mut tracks = Vec::new();
        for atom in Atoms::new(format, movie) {
            let atom = atom?;
            match &atom.kind {
                b"mvhd" => duration = Some(movie_duration(format, atom)?),
                b"trak" => {
                    if let Some(track) = track(format, atom, file)? {
                        tracks.push(track);
                    }
                }
                _ => {}
            }
        }
        let color_at = tracks
            .iter()
            .position(|track| track.handler == PICTURES && track.auxiliary_of.is_empty())
            .ok_or_else(|| Error::malformed(format, "sequence without a picture track"))?;
        let color = tracks.swap_remove(color_at);
        let alpha = tracks
            .into_iter()
            .find(|track| track.handler == AUXILIARY && track.auxiliary_of.contains(&color.id));
        if let Some(alpha) = &alpha
            && ((alpha.width, alpha.height) != (color.width, color.height)
                || alpha.samples.len() != color.samples.len())
        {
            return Err(Error::unsupported(
                format,
                "alpha track that differs from its pictures",
            ));
        }
        let plays = match (color.edit, duration) {
            (None, _) => None,
            (Some(edit), _) if !edit.repeats => Some(1),
            (Some(_), None | Some(Duration::Indefinite)) => None,
            (Some(edit), Some(Duration::Finite(total))) => match edit.segment {
                0 => None,
                segment => u32::try_from(total.div_ceil(segment))
                    .ok()
                    .map(|plays| plays.max(1)),
            },
        };
        Ok(Sequence {
            color,
            alpha,
            plays,
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum Duration {
    Finite(u64),
    Indefinite,
}

fn movie_duration(format: Format, atom: Atom<'_>) -> Result<Duration> {
    let (version, _, mut bytes) = atom.full(format)?;
    let duration = match version {
        1 => {
            bytes.skip(20)?;
            match bytes.u64_be()? {
                u64::MAX => return Ok(Duration::Indefinite),
                duration => duration,
            }
        }
        _ => {
            bytes.skip(12)?;
            match bytes.u32_be()? {
                u32::MAX => return Ok(Duration::Indefinite),
                duration => u64::from(duration),
            }
        }
    };
    Ok(Duration::Finite(duration))
}

#[derive(Default)]
struct Tables<'d> {
    times: Option<Atom<'d>>,
    sizes: Option<Atom<'d>>,
    chunks: Option<Atom<'d>>,
    offsets: Option<Atom<'d>>,
    wide_offsets: bool,
    entry: Option<Atom<'d>>,
}

fn track(format: Format, atom: Atom<'_>, file: u64) -> Result<Option<Track>> {
    let mut id = None;
    let mut auxiliary_of = Vec::new();
    let mut edit = None;
    let mut handler = [0; 4];
    let mut timescale = None;
    let mut tables = Tables::default();
    for child in Atoms::new(format, atom.body) {
        let child = child?;
        match &child.kind {
            b"tkhd" => {
                let (version, _, mut bytes) = child.full(format)?;
                bytes.skip(if version == 1 { 16 } else { 8 })?;
                id = Some(bytes.u32_be()?);
            }
            b"tref" => {
                for reference in Atoms::new(format, child.body) {
                    let reference = reference?;
                    if reference.kind == *b"auxl" {
                        let mut bytes = Bytes::new(format, reference.body);
                        while !bytes.is_empty() {
                            auxiliary_of.push(bytes.u32_be()?);
                        }
                    }
                }
            }
            b"edts" => {
                if let Some(list) = Atoms::find(format, child.body, *b"elst")? {
                    edit = Some(edit_of(format, list)?);
                }
            }
            b"mdia" => {
                for media in Atoms::new(format, child.body) {
                    let media = media?;
                    match &media.kind {
                        b"mdhd" => {
                            let (version, _, mut bytes) = media.full(format)?;
                            bytes.skip(if version == 1 { 16 } else { 8 })?;
                            timescale = Some(bytes.u32_be()?);
                        }
                        b"hdlr" => {
                            let (_, _, mut bytes) = media.full(format)?;
                            bytes.skip(4)?;
                            handler = bytes.fourcc()?;
                        }
                        b"minf" => tables = sample_tables(format, media)?,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    if handler != PICTURES && handler != AUXILIARY {
        return Ok(None);
    }
    let malformed = |part| Error::malformed(format, part);
    let id = id.ok_or_else(|| malformed("track without a header"))?;
    let timescale = timescale
        .filter(|&timescale| timescale > 0)
        .ok_or_else(|| malformed("track without a time scale"))?;
    let entry = tables
        .entry
        .ok_or_else(|| malformed("track without a sample entry"))?;
    let (coding, nclx, width, height) = sample_entry(format, entry)?;
    let samples = samples(format, &tables, file)?;
    Ok(Some(Track {
        id,
        coding,
        nclx,
        width,
        height,
        timescale,
        samples,
        handler,
        auxiliary_of,
        edit,
    }))
}

fn edit_of(format: Format, list: Atom<'_>) -> Result<Edit> {
    let (version, flags, mut bytes) = list.full(format)?;
    let count = bytes.u32_be()?;
    let segment = match (count, version) {
        (0, _) => 0,
        (_, 1) => bytes.u64_be()?,
        _ => u64::from(bytes.u32_be()?),
    };
    Ok(Edit {
        repeats: flags & REPEATS != 0,
        segment,
    })
}

fn sample_tables<'d>(format: Format, media: Atom<'d>) -> Result<Tables<'d>> {
    let mut tables = Tables::default();
    let Some(table) = Atoms::find(format, media.body, *b"stbl")? else {
        return Ok(tables);
    };
    for atom in Atoms::new(format, table.body) {
        let atom = atom?;
        match &atom.kind {
            b"stsd" => {
                let (_, _, mut bytes) = atom.full(format)?;
                bytes.skip(4)?;
                tables.entry = Atoms::new(format, bytes.rest()).next().transpose()?;
            }
            b"stts" => tables.times = Some(atom),
            b"stsz" => tables.sizes = Some(atom),
            b"stsc" => tables.chunks = Some(atom),
            b"stco" => tables.offsets = Some(atom),
            b"co64" => {
                tables.offsets = Some(atom);
                tables.wide_offsets = true;
            }
            _ => {}
        }
    }
    Ok(tables)
}

fn sample_entry(format: Format, entry: Atom<'_>) -> Result<(Coding, Option<Nclx>, u32, u32)> {
    let mut bytes = Bytes::new(format, entry.body);
    bytes.skip(24)?;
    let width = u32::from(bytes.u16_be()?);
    let height = u32::from(bytes.u16_be()?);
    bytes.seek(VISUAL_ENTRY_BYTES)?;
    let mut config = None;
    let mut nclx = None;
    for child in Atoms::new(format, bytes.rest()) {
        let child = child?;
        match (&child.kind, &entry.kind) {
            (b"av1C", b"av01") | (b"hvcC", b"hvc1" | b"hev1") => config = Some(child.body),
            (b"colr", _) => {
                let mut colour = Bytes::new(format, child.body);
                if colour.fourcc()? == *b"nclx" {
                    nclx = Some(Nclx {
                        primaries: colour.u16_be()?,
                        transfer: colour.u16_be()?,
                        matrix: colour.u16_be()?,
                        full_range: colour.u8()? & 0x80 != 0,
                    });
                }
            }
            _ => {}
        }
    }
    let config = config
        .ok_or_else(|| Error::malformed(format, "sample entry without a codec configuration"))?;
    if width == 0 || height == 0 {
        return Err(Error::InvalidSize { width, height });
    }
    Ok((
        Coding::new(format, entry.kind, config)?,
        nclx,
        width,
        height,
    ))
}

fn samples(format: Format, tables: &Tables<'_>, file: u64) -> Result<Vec<Sample>> {
    let malformed = |part| Error::malformed(format, part);
    let missing = || malformed("sample tables");
    let (_, _, mut sizes) = tables.sizes.ok_or_else(missing)?.full(format)?;
    let uniform = sizes.u32_be()?;
    let count = sizes.u32_be()? as usize;
    if count == 0 || count as u64 > file {
        return Err(malformed("sample count"));
    }
    let mut samples = Vec::with_capacity(count);
    for _ in 0..count {
        let size = match uniform {
            0 => sizes.u32_be()?,
            uniform => uniform,
        };
        if size == 0 {
            return Err(malformed("sample size"));
        }
        samples.push(Sample {
            offset: 0,
            size: u64::from(size),
            duration: 0,
        });
    }

    let (_, _, mut times) = tables.times.ok_or_else(missing)?.full(format)?;
    let mut sample = 0;
    for _ in 0..times.u32_be()? {
        let run = times.u32_be()? as usize;
        let duration = times.u32_be()?;
        for entry in samples.iter_mut().skip(sample).take(run) {
            entry.duration = duration;
        }
        sample = sample.saturating_add(run);
    }
    if sample < count {
        return Err(malformed("sample durations"));
    }

    let (_, _, mut offsets) = tables.offsets.ok_or_else(missing)?.full(format)?;
    let chunk_count = offsets.u32_be()? as usize;
    if chunk_count > offsets.remaining() {
        return Err(malformed("chunk offsets"));
    }
    let mut chunk_offsets = Vec::with_capacity(chunk_count);
    for _ in 0..chunk_count {
        chunk_offsets.push(match tables.wide_offsets {
            true => offsets.u64_be()?,
            false => u64::from(offsets.u32_be()?),
        });
    }

    let (_, _, mut chunks) = tables.chunks.ok_or_else(missing)?.full(format)?;
    let runs = chunks.u32_be()? as usize;
    if runs > chunks.remaining() {
        return Err(malformed("sample chunks"));
    }
    let mut spans = Vec::with_capacity(runs);
    for _ in 0..runs {
        let first = chunks.u32_be()? as usize;
        let per_chunk = chunks.u32_be()? as usize;
        chunks.skip(4)?;
        spans.push((first, per_chunk));
    }
    let mut next = 0;
    for (index, &(first, per_chunk)) in spans.iter().enumerate() {
        let last = spans
            .get(index + 1)
            .map_or(chunk_count, |&(following, _)| following.saturating_sub(1));
        if first == 0 || first > last + 1 {
            return Err(malformed("sample chunks"));
        }
        for chunk in first..=last {
            let mut offset = *chunk_offsets
                .get(chunk - 1)
                .ok_or_else(|| malformed("sample chunks"))?;
            for _ in 0..per_chunk {
                let Some(entry) = samples.get_mut(next) else {
                    break;
                };
                entry.offset = offset;
                offset = offset
                    .checked_add(entry.size)
                    .filter(|&end| end <= file)
                    .ok_or(Error::Truncated(format))?;
                next += 1;
            }
        }
    }
    if next < count {
        return Err(malformed("sample chunks"));
    }
    Ok(samples)
}
