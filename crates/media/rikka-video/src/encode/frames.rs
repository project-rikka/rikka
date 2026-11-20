use crate::error::{Result, check};
use rikka_ffmpeg::BufferRef;
use rikka_ffmpeg::Frame;
use rikka_ffmpeg::avutil::{self, AVPixelFormat};
use rikka_video_core::Picture;
use rikka_video_core::sw::SwConverter;

pub(crate) struct Frames {
    width: i32,
    height: i32,
    format: AVPixelFormat,
    upload: Option<BufferRef>,
    converter: SwConverter,
    staged: Frame,
    uploaded: Frame,
}

impl Frames {
    pub(crate) fn new(
        (width, height): (u32, u32),
        format: AVPixelFormat,
        upload: Option<BufferRef>,
    ) -> Result<Self> {
        Ok(Self {
            width: width as i32,
            height: height as i32,
            format,
            upload,
            converter: SwConverter::new(),
            staged: Frame::new()?,
            uploaded: Frame::new()?,
        })
    }

    pub(crate) fn prepare(
        &mut self,
        picture: Picture<'_>,
        place: i64,
        keyframe: bool,
    ) -> Result<&Frame> {
        self.stage()?;
        self.converter.fill(&mut self.staged, picture)?;
        let frame = match &self.upload {
            None => &mut self.staged,
            Some(surfaces) => {
                self.uploaded.unref();
                let got = unsafe {
                    avutil::av_hwframe_get_buffer(surfaces.as_ptr(), self.uploaded.as_mut_ptr(), 0)
                };
                check("av_hwframe_get_buffer", got)?;
                let transferred = unsafe {
                    avutil::av_hwframe_transfer_data(
                        self.uploaded.as_mut_ptr(),
                        self.staged.as_ptr(),
                        0,
                    )
                };
                check("av_hwframe_transfer_data", transferred)?;
                &mut self.uploaded
            }
        };
        unsafe {
            let raw = frame.as_mut_ptr();
            (*raw).pts = place;
            (*raw).pict_type = match keyframe {
                true => avutil::AV_PICTURE_TYPE_I,
                false => avutil::AV_PICTURE_TYPE_NONE,
            };
        }
        Ok(frame)
    }

    fn stage(&mut self) -> Result<()> {
        self.staged.unref();
        unsafe {
            let frame = self.staged.as_mut_ptr();
            (*frame).format = self.format;
            (*frame).width = self.width;
            (*frame).height = self.height;
            (*frame).colorspace = avutil::AVCOL_SPC_BT709;
            (*frame).color_range = avutil::AVCOL_RANGE_MPEG;
            let allocated = avutil::av_frame_get_buffer(frame, 0);
            check("av_frame_get_buffer", allocated)?;
        }
        Ok(())
    }
}
