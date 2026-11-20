#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    QrCode,
    MicroQrCode,
    RmqrCode,
    DataMatrix,
    Aztec,
    AztecRune,
    Pdf417,
    CompactPdf417,
    MicroPdf417,
    MaxiCode,
    Ean13,
    Ean8,
    UpcA,
    UpcE,
    Isbn,
    EanUpc,
    Code128,
    Code39,
    Code32,
    Pzn,
    Code93,
    Codabar,
    Itf,
    Itf14,
    DataBar,
    DataBarLimited,
    DataBarExpanded,
    Telepen,
    DxFilmEdge,
    Other,
}

impl Format {
    pub(crate) fn from_zxing(id: i32) -> Format {
        let symbology = (id & 0xff) as u8;
        let variant = ((id >> 8) & 0xff) as u8;
        match (symbology, variant) {
            (b'Q', b'm') => Format::MicroQrCode,
            (b'Q', b'r') => Format::RmqrCode,
            (b'Q', _) => Format::QrCode,
            (b'd', _) => Format::DataMatrix,
            (b'z', b'r') => Format::AztecRune,
            (b'z', _) => Format::Aztec,
            (b'L', b'c') => Format::CompactPdf417,
            (b'L', b'm') => Format::MicroPdf417,
            (b'L', _) => Format::Pdf417,
            (b'U', _) => Format::MaxiCode,
            (b'E', b'1') => Format::Ean13,
            (b'E', b'8') => Format::Ean8,
            (b'E', b'a') => Format::UpcA,
            (b'E', b'e') => Format::UpcE,
            (b'E', b'i') => Format::Isbn,
            (b'E', _) => Format::EanUpc,
            (b'C', _) => Format::Code128,
            (b'A', b'2') => Format::Code32,
            (b'A', b'p') => Format::Pzn,
            (b'A', _) => Format::Code39,
            (b'G', _) => Format::Code93,
            (b'F', _) => Format::Codabar,
            (b'I', b'4') => Format::Itf14,
            (b'I', _) => Format::Itf,
            (b'e', b'l') => Format::DataBarLimited,
            (b'e', b'e' | b'E') => Format::DataBarExpanded,
            (b'e', _) => Format::DataBar,
            (b'B', _) => Format::Telepen,
            (b'X', b'x') => Format::DxFilmEdge,
            _ => Format::Other,
        }
    }

    #[must_use]
    pub fn name(self) -> Option<&'static str> {
        Some(match self {
            Format::QrCode => "QR Code",
            Format::MicroQrCode => "Micro QR Code",
            Format::RmqrCode => "rMQR Code",
            Format::DataMatrix => "Data Matrix",
            Format::Aztec => "Aztec",
            Format::AztecRune => "Aztec Rune",
            Format::Pdf417 => "PDF417",
            Format::CompactPdf417 => "Compact PDF417",
            Format::MicroPdf417 => "MicroPDF417",
            Format::MaxiCode => "MaxiCode",
            Format::Ean13 => "EAN-13",
            Format::Ean8 => "EAN-8",
            Format::UpcA => "UPC-A",
            Format::UpcE => "UPC-E",
            Format::Isbn => "ISBN",
            Format::EanUpc => "EAN/UPC",
            Format::Code128 => "Code 128",
            Format::Code39 => "Code 39",
            Format::Code32 => "Code 32",
            Format::Pzn => "PZN",
            Format::Code93 => "Code 93",
            Format::Codabar => "Codabar",
            Format::Itf => "ITF",
            Format::Itf14 => "ITF-14",
            Format::DataBar => "DataBar",
            Format::DataBarLimited => "DataBar Limited",
            Format::DataBarExpanded => "DataBar Expanded",
            Format::Telepen => "Telepen",
            Format::DxFilmEdge => "DX Film Edge",
            Format::Other => return None,
        })
    }

    #[must_use]
    pub fn is_linear(self) -> bool {
        !matches!(
            self,
            Format::QrCode
                | Format::MicroQrCode
                | Format::RmqrCode
                | Format::DataMatrix
                | Format::Aztec
                | Format::AztecRune
                | Format::Pdf417
                | Format::CompactPdf417
                | Format::MicroPdf417
                | Format::MaxiCode
                | Format::Other
        )
    }
}
