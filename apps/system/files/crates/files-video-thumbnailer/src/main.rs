mod arguments;

use std::process::ExitCode;

use rikka_image::Png;
use rikka_video::PixelSize;

use crate::arguments::Arguments;

const NAME: &str = "rikka-video-thumbnailer";

fn main() -> ExitCode {
    let arguments = match Arguments::parse(std::env::args_os().skip(1)) {
        Ok(arguments) => arguments,
        Err(problem) => {
            eprintln!("{NAME}: {problem}");
            eprintln!("usage: {NAME} -s <size> <video> <thumbnail.png>");
            return ExitCode::from(2);
        }
    };
    match thumbnail(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("{NAME}: {}: {problem}", arguments.input.display());
            ExitCode::FAILURE
        }
    }
}

fn thumbnail(arguments: &Arguments) -> Result<(), Box<dyn std::error::Error>> {
    let picture = rikka_video::poster(&arguments.input)
        .with_max_size(PixelSize::new(arguments.size, arguments.size))
        .decode()?;
    picture.write(Png).to(&arguments.output)?;
    Ok(())
}
