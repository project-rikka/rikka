use moxcms::{
    CicpColorPrimaries, CicpProfile, ColorProfile, MatrixCoefficients, TransferCharacteristics,
};
use rikka_ffmpeg::Frame;
use rikka_ffmpeg::avutil;

use crate::error::{Error, Result};
use crate::format::Format;

pub(crate) fn profile(frame: &Frame, gray: bool) -> Result<Option<Vec<u8>>> {
    if let Some(profile) = frame.icc_profile() {
        return Ok(Some(profile.to_vec()));
    }
    let (primaries, transfer) = (frame.color_primaries(), frame.color_trc());
    let standard = matches!(
        primaries,
        avutil::AVCOL_PRI_BT709 | avutil::AVCOL_PRI_UNSPECIFIED
    ) && matches!(
        transfer,
        avutil::AVCOL_TRC_IEC61966_2_1 | avutil::AVCOL_TRC_UNSPECIFIED
    );
    match standard || gray {
        true => Ok(None),
        false => described(primaries, transfer),
    }
}

fn described(primaries: i32, transfer: i32) -> Result<Option<Vec<u8>>> {
    let primaries = u8::try_from(primaries)
        .ok()
        .and_then(|code| CicpColorPrimaries::try_from(code).ok());
    let transfer = u8::try_from(transfer)
        .ok()
        .and_then(|code| TransferCharacteristics::try_from(code).ok());
    let (Some(color_primaries), Some(transfer_characteristics)) = (primaries, transfer) else {
        return Ok(None);
    };
    let profile = ColorProfile::new_from_cicp(CicpProfile {
        color_primaries,
        transfer_characteristics,
        matrix_coefficients: MatrixCoefficients::Identity,
        full_range: true,
    });
    if profile.red_trc.is_none() {
        return Ok(None);
    }
    profile.encode().map(Some).map_err(|error| Error::Decoding {
        format: Format::JpegXl,
        message: format!("colour profile: {error}"),
    })
}
