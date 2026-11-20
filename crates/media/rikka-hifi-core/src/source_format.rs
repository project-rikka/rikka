use rikka_audio_decode::{AudioInfo, Codec, Encoding};

const UNDECLARED: Codec = Codec::Other("undeclared");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resolution {
    Pcm { rate: u32, bits: Option<u32> },
    Dsd { rate: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceFormat {
    pub encoding: Encoding,
    pub channels: u16,
    pub resolution: Resolution,
}

impl SourceFormat {
    #[must_use]
    pub fn pcm(rate: u32, channels: u16) -> SourceFormat {
        SourceFormat {
            encoding: encoded(UNDECLARED),
            channels: channels.max(1),
            resolution: Resolution::Pcm {
                rate: rate.max(1),
                bits: None,
            },
        }
    }

    #[must_use]
    pub fn dsd(rate: u32, channels: u16) -> SourceFormat {
        let rate = rate.max(1);
        SourceFormat {
            encoding: Encoding {
                dsd_rate: Some(rate),
                ..encoded(Codec::Dsd)
            },
            channels: channels.max(1),
            resolution: Resolution::Dsd { rate },
        }
    }

    #[must_use]
    pub fn bits(mut self, bits: u32) -> SourceFormat {
        if let Resolution::Pcm { rate, .. } = self.resolution {
            self.resolution = Resolution::Pcm {
                rate,
                bits: Some(bits),
            };
        }
        self
    }

    #[must_use]
    pub fn encoding(mut self, encoding: Encoding) -> SourceFormat {
        self.encoding = encoding;
        self
    }

    #[must_use]
    pub fn codec(mut self, codec: Codec) -> SourceFormat {
        self.encoding = Encoding {
            dsd_rate: self.encoding.dsd_rate,
            bitrate: self.encoding.bitrate,
            ..encoded(codec)
        };
        self
    }

    #[must_use]
    pub fn bitrate(mut self, bits_per_second: u32) -> SourceFormat {
        self.encoding.bitrate = Some(bits_per_second);
        self
    }

    #[must_use]
    pub fn rate(&self) -> u32 {
        match self.resolution {
            Resolution::Pcm { rate, .. } | Resolution::Dsd { rate } => rate,
        }
    }

    #[must_use]
    pub fn pcm_bits(&self) -> Option<u32> {
        match self.resolution {
            Resolution::Pcm { bits, .. } => bits,
            Resolution::Dsd { .. } => None,
        }
    }

    #[must_use]
    pub fn is_dsd(&self) -> bool {
        matches!(self.resolution, Resolution::Dsd { .. })
    }

    #[must_use]
    pub fn is_declared(&self) -> bool {
        !matches!(self.encoding.codec, Codec::Other(_))
    }

    #[must_use]
    pub fn is_lossy(&self) -> bool {
        self.is_declared() && !self.encoding.lossless
    }
}

impl From<&AudioInfo> for SourceFormat {
    fn from(info: &AudioInfo) -> SourceFormat {
        let format = SourceFormat::pcm(info.sample_rate, info.channels).encoding(info.encoding);
        match info.bits_per_sample {
            Some(bits) if info.encoding.lossless => format.bits(bits),
            _ => format,
        }
    }
}

fn encoded(codec: Codec) -> Encoding {
    Encoding {
        codec,
        lossless: codec.is_lossless(),
        bitrate: None,
        dsd_rate: None,
    }
}
