use crate::decoding::input::Input;
use crate::decoding::pipeline::Pipeline;
use crate::error::Result;
use crate::sound::Sound;

pub(crate) fn decode_all(input: Input) -> Result<Sound> {
    let mut decoder = Pipeline::open(input)?;
    let info = *decoder.info();
    let channels = usize::from(info.channels.max(1));
    let capacity = match (info.frames, info.duration) {
        (Some(frames), _) => frames as usize * channels,
        (None, Some(duration)) => {
            (duration.as_secs_f64() * f64::from(info.sample_rate)) as usize * channels
        }
        (None, None) => 0,
    };
    let mut samples = Vec::with_capacity(capacity);
    while decoder.next(&mut samples)? {}
    Ok(Sound::from_samples(
        samples,
        info.channels.max(1),
        info.sample_rate,
    ))
}
