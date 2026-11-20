use rikka_ffmpeg::avcodec::{self, AVCodecContext, AVCodecParameters};
#[cfg(unix)]
use rikka_ffmpeg::avutil;
use rikka_ffmpeg::avutil::AVRational;

#[cfg(unix)]
use crate::encode::VideoCodec;
use crate::error::{Error, Result, check};

pub struct StreamInfo {
    params: *mut AVCodecParameters,
    time_base: AVRational,
    frame_rate: Option<AVRational>,
}

unsafe impl Send for StreamInfo {}
unsafe impl Sync for StreamInfo {}

impl StreamInfo {
    pub(crate) fn from_context(
        context: *const AVCodecContext,
        time_base: AVRational,
        frame_rate: Option<AVRational>,
    ) -> Result<Self> {
        let made = StreamInfo::blank(time_base, frame_rate)?;
        let copied = unsafe { avcodec::avcodec_parameters_from_context(made.params, context) };
        check("avcodec_parameters_from_context", copied)?;
        Ok(made)
    }

    #[cfg(unix)]
    pub(crate) fn from_parameter_sets(
        codec: VideoCodec,
        (width, height): (u32, u32),
        parameter_sets: &[u8],
        time_base: AVRational,
        frame_rate: AVRational,
    ) -> Result<Self> {
        let codec_id = match codec {
            VideoCodec::H264 => avcodec::AV_CODEC_ID_H264,
            VideoCodec::Hevc => avcodec::AV_CODEC_ID_HEVC,
            VideoCodec::Av1 | VideoCodec::Vp9 => {
                return Err(Error::Unsupported(format!(
                    "{codec:?} streams carry no parameter sets"
                )));
            }
        };
        let made = StreamInfo::blank(time_base, Some(frame_rate))?;
        let padding = avcodec::AV_INPUT_BUFFER_PADDING_SIZE as usize;
        let extradata = unsafe { avutil::av_malloc(parameter_sets.len() + padding) }.cast::<u8>();
        if extradata.is_null() {
            return Err(Error::Unsupported("av_malloc failed".into()));
        }
        unsafe {
            std::ptr::copy_nonoverlapping(parameter_sets.as_ptr(), extradata, parameter_sets.len());
            extradata.add(parameter_sets.len()).write_bytes(0, padding);
            let params = &mut *made.params;
            params.codec_type = avutil::AVMEDIA_TYPE_VIDEO;
            params.codec_id = codec_id;
            params.format = avutil::AV_PIX_FMT_YUV420P;
            params.width = width as i32;
            params.height = height as i32;
            params.framerate = frame_rate;
            params.color_range = avutil::AVCOL_RANGE_MPEG;
            params.color_primaries = avutil::AVCOL_PRI_BT709;
            params.color_trc = avutil::AVCOL_TRC_BT709;
            params.color_space = avutil::AVCOL_SPC_BT709;
            params.extradata = extradata;
            params.extradata_size = parameter_sets.len() as i32;
        }
        Ok(made)
    }

    fn blank(time_base: AVRational, frame_rate: Option<AVRational>) -> Result<Self> {
        let params = unsafe { avcodec::avcodec_parameters_alloc() };
        if params.is_null() {
            return Err(Error::Unsupported("avcodec_parameters_alloc failed".into()));
        }
        Ok(StreamInfo {
            params,
            time_base,
            frame_rate,
        })
    }

    pub(crate) fn params(&self) -> *const AVCodecParameters {
        self.params
    }

    pub(crate) fn time_base(&self) -> AVRational {
        self.time_base
    }

    pub(crate) fn frame_rate(&self) -> Option<AVRational> {
        self.frame_rate
    }

    pub(crate) fn is_hevc(&self) -> bool {
        unsafe { (*self.params).codec_id == avcodec::AV_CODEC_ID_HEVC }
    }
}

impl Drop for StreamInfo {
    fn drop(&mut self) {
        unsafe { avcodec::avcodec_parameters_free(&mut self.params) };
    }
}

impl std::fmt::Debug for StreamInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, codec) = unsafe { ((*self.params).codec_type, (*self.params).codec_id) };
        f.debug_struct("StreamInfo")
            .field("kind", &kind)
            .field("codec", &codec)
            .field("time_base", &(self.time_base.num, self.time_base.den))
            .finish()
    }
}
