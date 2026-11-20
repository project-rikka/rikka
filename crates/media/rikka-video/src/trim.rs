use std::ops::Range;
use std::path::Path;
use std::ptr;
use std::time::Duration;

use crate::error::{Error, Result, check};
use crate::same_file::same_file;
use rikka_ffmpeg::OutputContext;
use rikka_ffmpeg::Packet;
use rikka_ffmpeg::avcodec;
use rikka_ffmpeg::avformat::{self, AVStream};
use rikka_ffmpeg::avutil::{self, AVRational};
use rikka_video_core::demux::Demuxer;
use rikka_video_core::init;

pub fn trim(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
    span: Range<Duration>,
) -> Result<Range<Duration>> {
    init::ensure()?;
    let source = source.as_ref();
    let destination = destination.as_ref();
    if same_file(source, destination) {
        return Err(Error::OverSource);
    }
    let requested = requested_range(&span)?;

    let mut input = Demuxer::open(source)?;
    if input
        .duration_us()
        .is_some_and(|duration| requested.start >= duration)
    {
        return Err(Error::EmptyCut);
    }
    let start = first_keyframe(&mut input, requested.start)?;
    if requested.end <= start {
        return Err(Error::EmptyCut);
    }
    let range = start..requested.end;

    let mut output = Output::open(destination, &input)?;
    input.seek_us(range.start)?;
    let ended = copy(&mut input, &mut output, &range)?;
    output.finish()?;

    let range = Duration::from_micros(range.start as u64)..Duration::from_micros(ended as u64);
    tracing::info!(
        "trimmed {} from {:?} to {:?} into {}",
        source.display(),
        range.start,
        range.end,
        destination.display()
    );
    Ok(range)
}

fn requested_range(span: &Range<Duration>) -> Result<Range<i64>> {
    let micros = |time: Duration| time.as_micros().min(i64::MAX as u128) as i64;
    let requested_range = micros(span.start)..micros(span.end);
    match requested_range.end > requested_range.start {
        true => Ok(requested_range),
        false => Err(Error::EmptyCut),
    }
}

fn first_keyframe(input: &mut Demuxer, target_us: i64) -> Result<i64> {
    input.seek_us(target_us)?;
    let time_base = input.time_base();
    let mut packet = Packet::new()?;

    while input.read(&mut packet)? {
        let stamp = packet.is_key().then(|| packet.stamp()).flatten();
        packet.unref();
        if let Some(stamp) = stamp {
            return Ok(rescaled(stamp, time_base, avutil::AV_TIME_BASE_Q).max(0));
        }
    }
    Err(Error::EmptyCut)
}

fn copy(input: &mut Demuxer, output: &mut Output, range: &Range<i64>) -> Result<i64> {
    let mut packet = Packet::new()?;
    let mut ended = None;

    while input.read_any(&mut packet)? {
        let Some(mapped) = output.mapped(packet.stream_index()) else {
            packet.unref();
            continue;
        };
        let micros = |stamp| rescaled(stamp, mapped.source, avutil::AV_TIME_BASE_Q);
        let Some(shown) = packet.stamp().map(micros) else {
            packet.unref();
            continue;
        };
        if shown >= range.end {
            output.ended(packet.stream_index());
            packet.unref();
            if output.is_ended() {
                break;
            }
            continue;
        }
        let mut span = micros(packet.span());
        if shown + span <= range.start {
            packet.unref();
            continue;
        }
        if shown + span > range.end {
            span = range.end - shown;
            packet.set_span(rescaled(span, avutil::AV_TIME_BASE_Q, mapped.source));
        }

        packet.shift(rescaled(range.start, avutil::AV_TIME_BASE_Q, mapped.source));
        output.write(&mut packet, mapped)?;
        packet.unref();
        if mapped.pictures {
            ended = Some(ended.unwrap_or(shown).max(shown + span));
        }
    }

    ended.ok_or(Error::EmptyCut)
}

fn rescaled(stamp: i64, from: AVRational, to: AVRational) -> i64 {
    unsafe { avutil::av_rescale_q(stamp, from, to) }
}

#[derive(Clone, Copy, Debug)]
struct Mapped {
    index: i32,
    source: AVRational,
    target: AVRational,
    pictures: bool,
    ended: bool,
}

struct Output {
    ctx: OutputContext,
    mapped: Vec<Option<Mapped>>,
}

impl Output {
    fn open(path: &Path, input: &Demuxer) -> Result<Output> {
        let ctx = OutputContext::create(path)?;
        unsafe {
            let copied =
                avutil::av_dict_copy(&raw mut (*ctx.as_ptr()).metadata, input.metadata(), 0);
            check("av_dict_copy", copied)?;
        }
        let mut output = Output {
            ctx,
            mapped: Vec::with_capacity(input.stream_count()),
        };

        for index in 0..input.stream_count() {
            let stream = input.stream_at(index);
            let carried = output.map_stream(stream, index as i32 == input.video_index())?;
            output.mapped.push(carried);
        }
        if !output.has_video() {
            return Err(Error::Unsupported("no video stream to trim".into()));
        }
        output.begin()?;
        Ok(output)
    }

    fn map_stream(&mut self, stream: &AVStream, pictures: bool) -> Result<Option<Mapped>> {
        let kind = unsafe { (*stream.codecpar).codec_type };
        let carried = matches!(
            kind,
            avutil::AVMEDIA_TYPE_VIDEO | avutil::AVMEDIA_TYPE_AUDIO | avutil::AVMEDIA_TYPE_SUBTITLE
        );
        if !carried || stream.disposition & avformat::AV_DISPOSITION_ATTACHED_PIC != 0 {
            return Ok(None);
        }

        let created = unsafe { avformat::avformat_new_stream(self.ctx.as_ptr(), ptr::null()) };
        if created.is_null() {
            return Err(Error::Unsupported("avformat_new_stream failed".into()));
        }
        let index = unsafe {
            let copied = avcodec::avcodec_parameters_copy((*created).codecpar, stream.codecpar);
            check("avcodec_parameters_copy", copied)?;
            (*(*created).codecpar).codec_tag = 0;
            (*created).time_base = stream.time_base;
            (*created).disposition = stream.disposition;
            (*created).avg_frame_rate = stream.avg_frame_rate;
            (*created).r_frame_rate = stream.r_frame_rate;
            (*created).sample_aspect_ratio = stream.sample_aspect_ratio;
            let copied = avutil::av_dict_copy(&raw mut (*created).metadata, stream.metadata, 0);
            check("av_dict_copy", copied)?;
            (*created).index
        };

        Ok(Some(Mapped {
            index,
            source: stream.time_base,
            target: stream.time_base,
            pictures,
            ended: false,
        }))
    }

    fn begin(&mut self) -> Result<()> {
        self.ctx.write_header(false)?;
        for mapped in self.mapped.iter_mut().flatten() {
            mapped.target =
                unsafe { (**(*self.ctx.as_ptr()).streams.add(mapped.index as usize)).time_base };
        }
        Ok(())
    }

    fn mapped(&self, from: i32) -> Option<Mapped> {
        let mapped = usize::try_from(from)
            .ok()
            .and_then(|from| self.mapped.get(from))
            .copied()
            .flatten()?;
        (!mapped.ended).then_some(mapped)
    }

    fn ended(&mut self, from: i32) {
        if let Some(mapped) = usize::try_from(from)
            .ok()
            .and_then(|from| self.mapped.get_mut(from))
            .and_then(Option::as_mut)
        {
            mapped.ended = true;
        }
    }

    fn is_ended(&self) -> bool {
        self.mapped.iter().flatten().all(|mapped| mapped.ended)
    }

    fn has_video(&self) -> bool {
        self.mapped.iter().flatten().any(|mapped| mapped.pictures)
    }

    fn write(&mut self, packet: &mut Packet, mapped: Mapped) -> Result<()> {
        packet.rescale(mapped.source, mapped.target);
        packet.set_stream_index(mapped.index);
        packet.forget_place();
        Ok(self.ctx.write(packet, "av_interleaved_write_frame")?)
    }

    fn finish(mut self) -> Result<()> {
        Ok(self.ctx.write_trailer()?)
    }
}
