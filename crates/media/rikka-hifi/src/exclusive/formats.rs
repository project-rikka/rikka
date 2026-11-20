use alsa::pcm::Format;
use rikka_hifi_core::SampleFormat;

pub(crate) const PLAYABLE: [SampleFormat; 11] = [
    SampleFormat::S16,
    SampleFormat::S24Packed,
    SampleFormat::S24,
    SampleFormat::S32,
    SampleFormat::F32,
    SampleFormat::F64,
    SampleFormat::Dsd8,
    SampleFormat::Dsd16Le,
    SampleFormat::Dsd16Be,
    SampleFormat::Dsd32Le,
    SampleFormat::Dsd32Be,
];

pub(crate) fn alsa(format: SampleFormat) -> Format {
    match format {
        SampleFormat::S16 => Format::S16LE,
        SampleFormat::S24Packed => Format::S243LE,
        SampleFormat::S24 => Format::S24LE,
        SampleFormat::S32 => Format::S32LE,
        SampleFormat::F32 => Format::FloatLE,
        SampleFormat::F64 => Format::Float64LE,
        SampleFormat::Dsd8 => Format::DSDU8,
        SampleFormat::Dsd16Le => Format::DSDU16LE,
        SampleFormat::Dsd16Be => Format::DSDU16BE,
        SampleFormat::Dsd32Le => Format::DSDU32LE,
        SampleFormat::Dsd32Be => Format::DSDU32BE,
    }
}

pub(crate) fn named(name: &str) -> Option<SampleFormat> {
    PLAYABLE
        .into_iter()
        .find(|format| format.to_string() == name)
}
