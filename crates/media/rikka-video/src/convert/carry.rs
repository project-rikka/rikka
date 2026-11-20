use std::ptr;

use rikka_ffmpeg::avcodec::{self, AVCodecParameters, AVPacketSideDataType};
use rikka_ffmpeg::avformat::{self, AVStream};
use rikka_ffmpeg::avutil::{self, AVRational};
use rikka_ffmpeg::{OutputContext, Packet};

use crate::error::{Error, Result, check};

const PRESENTATION: [AVPacketSideDataType; 5] = [
    avcodec::AV_PKT_DATA_DISPLAYMATRIX,
    avcodec::AV_PKT_DATA_STEREO3D,
    avcodec::AV_PKT_DATA_MASTERING_DISPLAY_METADATA,
    avcodec::AV_PKT_DATA_SPHERICAL,
    avcodec::AV_PKT_DATA_CONTENT_LIGHT_LEVEL,
];

pub(crate) struct CopiedStream {
    pub(crate) index: i32,
    pub(crate) from: AVRational,
    pub(crate) to: AVRational,
    pub(crate) shift: i64,
}

impl CopiedStream {
    pub(crate) fn open(
        stream: &AVStream,
        output: &OutputContext,
        start_us: i64,
    ) -> Result<CopiedStream> {
        let created = new_stream(output)?;
        let index = unsafe {
            let copied = avcodec::avcodec_parameters_copy((*created).codecpar, stream.codecpar);
            check("avcodec_parameters_copy", copied)?;
            (*(*created).codecpar).codec_tag = 0;
            describe(created, stream)?;
            tag_for(output, created);
            (*created).index
        };
        Ok(CopiedStream {
            index,
            from: stream.time_base,
            to: stream.time_base,
            shift: shift(start_us, stream.time_base),
        })
    }

    pub(crate) fn update_time_base(&mut self, output: &OutputContext) {
        self.to = output_time_base(output, self.index);
    }

    pub(crate) fn write(&self, packet: &mut Packet, output: &mut OutputContext) -> Result<()> {
        packet.shift(self.shift);
        packet.rescale(self.from, self.to);
        packet.set_stream_index(self.index);
        packet.forget_place();
        Ok(output.write(packet, "av_interleaved_write_frame(copy)")?)
    }
}

pub(crate) fn new_stream(output: &OutputContext) -> Result<*mut AVStream> {
    let created = unsafe { avformat::avformat_new_stream(output.as_ptr(), ptr::null()) };
    match created.is_null() {
        true => Err(Error::Unsupported("avformat_new_stream failed".into())),
        false => Ok(created),
    }
}

pub(crate) unsafe fn describe(created: *mut AVStream, stream: &AVStream) -> Result<()> {
    unsafe {
        (*created).time_base = stream.time_base;
        (*created).disposition = stream.disposition;
        (*created).avg_frame_rate = stream.avg_frame_rate;
        (*created).r_frame_rate = stream.r_frame_rate;
        (*created).sample_aspect_ratio = stream.sample_aspect_ratio;
        let copied = avutil::av_dict_copy(&raw mut (*created).metadata, stream.metadata, 0);
        check("av_dict_copy", copied)?;
    }
    Ok(())
}

pub(crate) unsafe fn presentation(from: *const AVCodecParameters, to: *mut AVCodecParameters) {
    unsafe {
        for index in 0..(*from).nb_coded_side_data.max(0) as usize {
            let side = &*(*from).coded_side_data.add(index);
            if !PRESENTATION.contains(&side.r#type) || side.data.is_null() {
                continue;
            }
            let made = avcodec::av_packet_side_data_new(
                &raw mut (*to).coded_side_data,
                &raw mut (*to).nb_coded_side_data,
                side.r#type,
                side.size,
                0,
            );
            if !made.is_null() {
                ptr::copy_nonoverlapping(side.data, (*made).data, side.size);
            }
        }
    }
}

pub(crate) fn output_time_base(output: &OutputContext, index: i32) -> AVRational {
    unsafe { (**(*output.as_ptr()).streams.add(index as usize)).time_base }
}

pub(crate) fn shift(start_us: i64, time_base: AVRational) -> i64 {
    unsafe { avutil::av_rescale_q(start_us, avutil::AV_TIME_BASE_Q, time_base) }
}

pub(crate) unsafe fn tag_for(output: &OutputContext, created: *mut AVStream) {
    unsafe {
        let parameters = (*created).codecpar;
        if output.is_quicktime() && (*parameters).codec_id == avcodec::AV_CODEC_ID_HEVC {
            (*parameters).codec_tag = u32::from_le_bytes(*b"hvc1");
        }
    }
}
