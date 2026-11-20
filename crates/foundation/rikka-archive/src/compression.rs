use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Compression {
    Stored,
    Deflate,
    Deflate64,
    Bzip2,
    Lzma,
    Lzma2,
    Xz,
    Zstd,
    Ppmd,
    Brotli,
    Lz4,
    Implode,
    Shrink,
    Reduce,
    Unknown,
}

impl Compression {
    pub(crate) fn from_zip(method: zip::CompressionMethod) -> Compression {
        use zip::CompressionMethod as Method;

        match method {
            Method::Stored => Compression::Stored,
            Method::Deflated => Compression::Deflate,
            Method::Deflate64 => Compression::Deflate64,
            Method::Bzip2 => Compression::Bzip2,
            Method::Zstd => Compression::Zstd,
            Method::Lzma => Compression::Lzma,
            Method::Xz => Compression::Xz,
            Method::Ppmd => Compression::Ppmd,
            Method::Shrink => Compression::Shrink,
            Method::Reduce(_) => Compression::Reduce,
            Method::Implode => Compression::Implode,
            _ => Compression::Unknown,
        }
    }

    pub(crate) fn from_seven_z(methods: &[sevenz_rust2::EncoderMethod]) -> Compression {
        use sevenz_rust2::EncoderMethod as Method;

        methods
            .iter()
            .find_map(|&method| match method {
                Method::COPY => Some(Compression::Stored),
                Method::LZMA => Some(Compression::Lzma),
                Method::LZMA2 => Some(Compression::Lzma2),
                Method::BZIP2 => Some(Compression::Bzip2),
                Method::ZSTD => Some(Compression::Zstd),
                Method::PPMD => Some(Compression::Ppmd),
                Method::BROTLI => Some(Compression::Brotli),
                Method::LZ4 => Some(Compression::Lz4),
                Method::DEFLATE => Some(Compression::Deflate),
                Method::DEFLATE64 => Some(Compression::Deflate64),
                _ => None,
            })
            .unwrap_or(Compression::Unknown)
    }
}

impl fmt::Display for Compression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Compression::Stored => "stored",
            Compression::Deflate => "deflate",
            Compression::Deflate64 => "deflate64",
            Compression::Bzip2 => "bzip2",
            Compression::Lzma => "LZMA",
            Compression::Lzma2 => "LZMA2",
            Compression::Xz => "XZ",
            Compression::Zstd => "zstd",
            Compression::Ppmd => "PPMd",
            Compression::Brotli => "brotli",
            Compression::Lz4 => "LZ4",
            Compression::Implode => "implode",
            Compression::Shrink => "shrink",
            Compression::Reduce => "reduce",
            Compression::Unknown => "unknown",
        };
        f.write_str(name)
    }
}
