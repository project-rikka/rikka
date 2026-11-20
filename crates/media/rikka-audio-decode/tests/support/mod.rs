pub(crate) fn generate_wav(sample_rate: u32, frames: usize) -> Vec<u8> {
    let channels = 2u16;
    let bits = 16u16;
    let byte_rate = sample_rate * channels as u32 * (bits / 8) as u32;
    let block_align = channels * (bits / 8);
    let data_len = (frames * channels as usize * 2) as u32;

    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for n in 0..frames {
        for frequency in [440.0f64, 880.0] {
            let value = (std::f64::consts::TAU * frequency * n as f64 / sample_rate as f64).sin();
            wav.extend_from_slice(&((value * 30000.0) as i16).to_le_bytes());
        }
    }
    wav
}
