use std::ffi::OsString;
use std::path::PathBuf;

const LARGEST: u32 = 4096;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Arguments {
    pub(crate) size: u32,
    pub(crate) input: PathBuf,
    pub(crate) output: PathBuf,
}

impl Arguments {
    pub(crate) fn parse(
        arguments: impl IntoIterator<Item = OsString>,
    ) -> Result<Arguments, String> {
        let mut size = None;
        let mut paths = Vec::new();
        let mut options_ended = false;
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            if options_ended {
                paths.push(PathBuf::from(argument));
                continue;
            }
            match argument.to_str() {
                Some("--") => options_ended = true,
                Some("-s" | "--size") => {
                    let given = arguments.next().ok_or("-s needs a size")?;
                    size = Some(parsed_size(&given)?);
                }
                Some(option) if option.starts_with('-') && option.len() > 1 => {
                    return Err(format!("unknown option {option}"));
                }
                _ => paths.push(PathBuf::from(argument)),
            }
        }
        let [input, output] = <[PathBuf; 2]>::try_from(paths)
            .map_err(|_| "one video and one thumbnail path are needed".to_owned())?;
        Ok(Arguments {
            size: size.ok_or("a size is needed (-s)")?,
            input,
            output,
        })
    }
}

fn parsed_size(given: &OsString) -> Result<u32, String> {
    given
        .to_str()
        .and_then(|text| text.parse::<u32>().ok())
        .filter(|size| (1..=LARGEST).contains(size))
        .ok_or_else(|| format!("the size must be a number from 1 to {LARGEST}"))
}
