use core::ffi::{CStr, c_int};

use rikka_ffmpeg::avcodec::{
    self, AV_CODEC_PROP_LOSSLESS, AV_CODEC_PROP_LOSSY, AV_PROFILE_AAC_HE, AV_PROFILE_AAC_HE_V2,
    AVCodecID,
};

use crate::encoding::{Codec, Encoding};

const NAMED: &[(&str, Codec)] = &[
    ("flac", Codec::Flac),
    ("alac", Codec::Alac),
    ("wavpack", Codec::WavPack),
    ("ape", Codec::Ape),
    ("tak", Codec::Tak),
    ("tta", Codec::Tta),
    ("dst", Codec::Dsd),
    ("mp1", Codec::Mp1),
    ("mp3", Codec::Mp3),
    ("mp2", Codec::Mp2),
    ("aac", Codec::Aac),
    ("aac_latm", Codec::Aac),
    ("vorbis", Codec::Vorbis),
    ("opus", Codec::Opus),
    ("speex", Codec::Speex),
    ("musepack7", Codec::Musepack),
    ("musepack8", Codec::Musepack),
    ("wmav1", Codec::Wma),
    ("wmav2", Codec::Wma),
    ("wmapro", Codec::Wma),
    ("wmalossless", Codec::WmaLossless),
    ("ac3", Codec::Ac3),
    ("eac3", Codec::Eac3),
];

const PREFIXED: &[(&str, Codec)] = &[
    ("dsd_", Codec::Dsd),
    ("pcm_f", Codec::FloatPcm),
    ("pcm_", Codec::Pcm),
    ("adpcm_", Codec::Adpcm),
];

pub(crate) fn encoding(id: AVCodecID, profile: c_int, sniffed: Option<Encoding>) -> Encoding {
    let descriptor = unsafe { avcodec::avcodec_descriptor_get(id) };
    let (name, props) = match descriptor.is_null() {
        true => ("unknown", 0),
        false => unsafe {
            let name = CStr::from_ptr((*descriptor).name)
                .to_str()
                .unwrap_or("unknown");
            (name, (*descriptor).props)
        },
    };
    let codec = NAMED
        .iter()
        .find(|(known, _)| *known == name)
        .or_else(|| PREFIXED.iter().find(|(prefix, _)| name.starts_with(prefix)))
        .map_or(Codec::Other(name), |(_, codec)| *codec);
    let codec = match (codec, profile) {
        (Codec::Aac, AV_PROFILE_AAC_HE) => Codec::HeAac,
        (Codec::Aac, AV_PROFILE_AAC_HE_V2) => Codec::HeAacV2,
        (codec, _) => codec,
    };
    let mut encoding = Encoding::for_codec(codec);
    encoding.lossless = match (codec, sniffed) {
        (Codec::WavPack, Some(sniffed)) => sniffed.lossless,
        (Codec::Other(_), _) => {
            props & AV_CODEC_PROP_LOSSLESS != 0 && props & AV_CODEC_PROP_LOSSY == 0
        }
        (codec, _) => codec.is_lossless(),
    };
    encoding
}
