use std::ptr;

use rikka_ffmpeg::avcodec::{self, AVCodec, AVCodecContext};
use rikka_ffmpeg::avutil::{self, AVRational};
use rikka_ffmpeg::{CodecContext, Frame, Packet, av_err, check};

use crate::error::{Error, Result};
use crate::format::Format;

const AV1_DECODERS: [&std::ffi::CStr; 2] = [c"libdav1d", c"libaom-av1"];
const JPEG_XL_DECODER: &std::ffi::CStr = c"libjxl";
const CODED_ALIGNMENT: u64 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decoding {
    Av1,
    Hevc,
    Tiff,
    Exr,
    Jpeg2000,
    JpegXl,
}

impl Decoding {
    const fn refuses_damage(self) -> bool {
        matches!(self, Decoding::Av1 | Decoding::Hevc)
    }

    const fn name(self) -> &'static str {
        match self {
            Decoding::Av1 => "AV1",
            Decoding::Hevc => "HEVC",
            Decoding::Tiff => "TIFF",
            Decoding::Exr => "OpenEXR",
            Decoding::Jpeg2000 => "JPEG 2000",
            Decoding::JpegXl => "JPEG XL",
        }
    }

    fn find(self) -> *const AVCodec {
        let by_id = |id| unsafe { avcodec::avcodec_find_decoder(id) };
        match self {
            Decoding::Av1 => AV1_DECODERS
                .iter()
                .map(|name| unsafe { avcodec::avcodec_find_decoder_by_name(name.as_ptr()) })
                .find(|codec| !codec.is_null())
                .unwrap_or(ptr::null()),
            Decoding::Hevc => by_id(avcodec::AV_CODEC_ID_HEVC),
            Decoding::Tiff => by_id(avcodec::AV_CODEC_ID_TIFF),
            Decoding::Exr => by_id(avcodec::AV_CODEC_ID_EXR),
            Decoding::Jpeg2000 => by_id(avcodec::AV_CODEC_ID_JPEG2000),
            Decoding::JpegXl => unsafe {
                avcodec::avcodec_find_decoder_by_name(JPEG_XL_DECODER.as_ptr())
            },
        }
    }
}

pub(crate) fn load(format: Format) -> Result<()> {
    rikka_ffmpeg::load_scaling().map_err(|error| Error::decoding(format, error))
}

#[derive(Debug)]
pub(crate) struct Decoder {
    format: Format,
    ctx: CodecContext,
    frame: Frame,
    packet: Packet,
    flushed: bool,
}

impl Decoder {
    pub(crate) fn open(
        format: Format,
        decoding: Decoding,
        config: &[u8],
        size: (u32, u32),
    ) -> Result<Decoder> {
        Decoder::open_with(format, decoding, config, size, |_| {})
    }

    pub(crate) fn open_timed(
        format: Format,
        decoding: Decoding,
        size: (u32, u32),
        time_base: AVRational,
    ) -> Result<Decoder> {
        Decoder::open_with(format, decoding, &[], size, |ctx| {
            ctx.pkt_timebase = time_base;
        })
    }

    fn open_with(
        format: Format,
        decoding: Decoding,
        config: &[u8],
        (width, height): (u32, u32),
        configure: impl FnOnce(&mut AVCodecContext),
    ) -> Result<Decoder> {
        load(format)?;
        let failed = |error| Error::decoding(format, error);
        let codec = decoding.find();
        if codec.is_null() {
            return Err(Error::NoDecoder(decoding.name()));
        }
        let ctx = unsafe { CodecContext::alloc(codec) }.map_err(failed)?;
        let raw = ctx.as_ptr();
        let pixels = (u64::from(width) + CODED_ALIGNMENT) * (u64::from(height) + CODED_ALIGNMENT);
        unsafe {
            if !config.is_empty() {
                let padded = config.len() + avcodec::AV_INPUT_BUFFER_PADDING_SIZE as usize;
                let extradata = avutil::av_malloc(padded).cast::<u8>();
                if extradata.is_null() {
                    return Err(failed(av_err("av_malloc", avutil::AVERROR_ENOMEM)));
                }
                extradata.write_bytes(0, padded);
                extradata.copy_from_nonoverlapping(config.as_ptr(), config.len());
                (*raw).extradata = extradata;
                (*raw).extradata_size = config.len() as i32;
            }
            (*raw).max_pixels = pixels.min(i64::MAX as u64) as i64;
            (*raw).thread_count = 0;
            (*raw).thread_type = avcodec::FF_THREAD_SLICE;
            if decoding.refuses_damage() {
                (*raw).err_recognition |= avcodec::AV_EF_EXPLODE;
            }
            (*raw).flags |= avcodec::AV_CODEC_FLAG_LOW_DELAY;
            configure(&mut *raw);
            check(
                "avcodec_open2",
                avcodec::avcodec_open2(raw, codec, ptr::null_mut()),
            )
            .map_err(failed)?;
        }
        Ok(Decoder {
            format,
            ctx,
            frame: Frame::new().map_err(failed)?,
            packet: Packet::new().map_err(failed)?,
            flushed: false,
        })
    }

    pub(crate) fn next<'p>(
        &mut self,
        packets: &mut impl Iterator<Item = (i64, &'p [u8])>,
    ) -> Result<Option<&Frame>> {
        loop {
            let status = unsafe {
                avcodec::avcodec_receive_frame(self.ctx.as_ptr(), self.frame.as_mut_ptr())
            };
            match status {
                0 if self.frame.is_corrupt() => return Err(self.damaged("damaged picture")),
                0 => return Ok(Some(&self.frame)),
                avutil::AVERROR_EOF => return Ok(None),
                avutil::AVERROR_EAGAIN => {}
                code => {
                    return Err(Error::decoding(
                        self.format,
                        av_err("avcodec_receive_frame", code),
                    ));
                }
            }
            match packets.next() {
                Some((stamp, data)) => self.send(stamp, data)?,
                None if !self.flushed => {
                    self.flushed = true;
                    let status =
                        unsafe { avcodec::avcodec_send_packet(self.ctx.as_ptr(), ptr::null()) };
                    check("avcodec_send_packet", status)
                        .map_err(|error| Error::decoding(self.format, error))?;
                }
                None => return Ok(None),
            }
        }
    }

    fn send(&mut self, stamp: i64, data: &[u8]) -> Result<()> {
        let ctx = self.ctx.as_ptr();
        self.packet.place(stamp, stamp, 1, true);
        let status = self.packet.with_borrowed_data(data, |packet| unsafe {
            avcodec::avcodec_send_packet(ctx, packet.as_mut_ptr())
        });
        check("avcodec_send_packet", status)
            .map(|_| ())
            .map_err(|error| Error::decoding(self.format, error))
    }

    fn damaged(&self, problem: &str) -> Error {
        Error::Decoding {
            format: self.format,
            message: problem.to_owned(),
        }
    }
}
