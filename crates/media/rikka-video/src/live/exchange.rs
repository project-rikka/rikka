use rikka_ffmpeg::avcodec::AVCodecContext;
use rikka_ffmpeg::{CodecContext, Frame, Packet, avcodec, avutil};

use crate::error::{Error, Result, check};

pub(crate) unsafe fn refuse_damage(context: *mut AVCodecContext) {
    unsafe {
        (*context).flags = ((*context).flags | avcodec::AV_CODEC_FLAG_LOW_DELAY)
            & !avcodec::AV_CODEC_FLAG_OUTPUT_CORRUPT;
        (*context).err_recognition |= avcodec::AV_EF_CRCCHECK
            | avcodec::AV_EF_BITSTREAM
            | avcodec::AV_EF_BUFFER
            | avcodec::AV_EF_EXPLODE;
        (*context).error_concealment = 0;
    }
}

pub(crate) fn decode_packet(
    codec: &CodecContext,
    packet: &mut Packet,
    data: &[u8],
    frame: &mut Frame,
    mut each: impl FnMut(&mut Frame) -> Result<()>,
) -> Result<()> {
    if data.is_empty() {
        return Ok(());
    }
    loop {
        let context = codec.as_ptr();
        let sent = packet.with_borrowed_data(data, |packet| unsafe {
            avcodec::avcodec_send_packet(context, packet.as_mut_ptr())
        });
        let refused = sent == avutil::AVERROR_EAGAIN;
        if !refused {
            check("avcodec_send_packet", sent)?;
        }
        let drained = drain(codec, frame, &mut each)?;
        match (refused, drained) {
            (false, _) => return Ok(()),
            (true, 0) => {
                return Err(Error::Unsupported(
                    "the decoder neither took the packet nor gave a frame".into(),
                ));
            }
            (true, _) => {}
        }
    }
}

fn drain(
    codec: &CodecContext,
    frame: &mut Frame,
    each: &mut impl FnMut(&mut Frame) -> Result<()>,
) -> Result<usize> {
    let mut drained = 0;
    loop {
        let received =
            unsafe { avcodec::avcodec_receive_frame(codec.as_ptr(), frame.as_mut_ptr()) };
        if received == avutil::AVERROR_EAGAIN || received == avutil::AVERROR_EOF {
            return Ok(drained);
        }
        check("avcodec_receive_frame", received)?;
        if frame.is_damaged() {
            frame.unref();
            return Err(Error::Damaged);
        }
        drained += 1;
        each(frame)?;
    }
}
