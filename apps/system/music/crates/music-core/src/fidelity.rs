use crate::codec::Codec;
use crate::resolution::Resolution;

const DSD_BASES: [u32; 2] = [44_100, 48_000];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fidelity {
    pub codec: Codec,
    pub lossless: bool,
    pub resolution: Option<Resolution>,
    pub kbps: Option<u32>,
    pub dsd: Option<u32>,
}

impl Fidelity {
    #[must_use]
    pub fn lossless(codec: Codec, resolution: Resolution) -> Fidelity {
        Fidelity {
            codec,
            lossless: true,
            resolution: Some(resolution),
            kbps: None,
            dsd: None,
        }
    }

    #[must_use]
    pub fn lossy(codec: Codec, kbps: Option<u32>) -> Fidelity {
        Fidelity {
            codec,
            lossless: false,
            resolution: None,
            kbps,
            dsd: None,
        }
    }

    #[must_use]
    pub fn dsd(rate: u32) -> Fidelity {
        Fidelity {
            codec: Codec::Dsd,
            lossless: true,
            resolution: None,
            kbps: None,
            dsd: Some(rate),
        }
    }

    #[must_use]
    pub fn new(
        codec: Codec,
        lossless: bool,
        bits: Option<u32>,
        rate: Option<u32>,
        kbps: Option<u32>,
    ) -> Fidelity {
        match (codec, rate) {
            (Codec::Dsd, Some(rate)) => Fidelity::dsd(rate),
            _ if lossless => Fidelity {
                codec,
                lossless,
                resolution: bits.zip(rate).map(|(bits, rate)| Resolution { bits, rate }),
                kbps: None,
                dsd: None,
            },
            _ => Fidelity::lossy(codec, kbps),
        }
    }

    #[must_use]
    pub fn dsd_multiple(rate: u32) -> Option<u32> {
        DSD_BASES
            .into_iter()
            .find(|base| rate.is_multiple_of(*base))
            .map(|base| rate / base)
    }

    #[must_use]
    pub fn from_encoding(
        encoding: rikka_audio_decode::Encoding,
        rate: u32,
        bits: Option<u32>,
    ) -> Fidelity {
        if let Some(dsd) = encoding.dsd_rate {
            return Fidelity::dsd(dsd);
        }
        Fidelity {
            codec: encoding.codec.into(),
            lossless: encoding.lossless,
            resolution: bits
                .filter(|_| encoding.lossless)
                .map(|bits| Resolution { bits, rate }),
            kbps: encoding.bitrate.map(|bits| bits.div_ceil(1000)),
            dsd: None,
        }
    }

    #[must_use]
    pub fn from_source_format(format: &rikka_hifi::SourceFormat) -> Fidelity {
        Fidelity::from_encoding(format.encoding, format.rate(), format.pcm_bits())
    }

    #[must_use]
    pub fn is_high(&self) -> bool {
        self.dsd.is_some() || (self.lossless && self.resolution.is_some_and(Resolution::is_high))
    }
}
