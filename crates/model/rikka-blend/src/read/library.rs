use rikka_model_core::{Result, Warning};

use super::resources::Resources;
use crate::file::{Blend, LibraryFile, Source};

const MAX_LIBRARIES: usize = 64;

pub(crate) fn gather<'a>(
    main: &Blend<'_>,
    resources: &mut dyn Resources,
) -> Result<Vec<Blend<'a>>> {
    let limit = resources.max_file_bytes();
    let mut linked = Vec::new();
    let mut tried: Vec<String> = vec![main.uri().to_owned()];
    let mut pending = main.libraries();
    while !pending.is_empty() {
        for LibraryFile { uri, packed } in std::mem::take(&mut pending) {
            if tried.contains(&uri) {
                continue;
            }
            tried.push(uri.clone());
            if linked.len() >= MAX_LIBRARIES {
                resources.warn(Warning::Unsupported(format!(
                    "more than {MAX_LIBRARIES} linked libraries, skipping \"{uri}\""
                )));
                continue;
            }
            let source = match packed {
                Some(bytes) => {
                    resources.charge(bytes.len() as u64)?;
                    Source::Unpacked(bytes)
                }
                None => match resources.read_optional_resource(&uri)? {
                    Some(bytes) => Source::Shared(bytes),
                    None => continue,
                },
            };
            match Blend::open(source, limit) {
                Ok(blend) => {
                    let blend = blend.with_uri(uri);
                    pending.extend(blend.libraries());
                    linked.push(blend);
                }
                Err(error) => resources.warn(Warning::Unsupported(format!(
                    "failed to read linked library \"{uri}\": {error}"
                ))),
            }
        }
    }
    Ok(linked)
}
