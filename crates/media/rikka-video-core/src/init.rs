use crate::error::Result;

pub fn ensure() -> Result<()> {
    rikka_ffmpeg::load_scaling()?;
    Ok(())
}
