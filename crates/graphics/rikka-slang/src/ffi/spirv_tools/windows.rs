use libloading::os::windows::Library;

use super::SpirvTools;
use crate::ffi::load_beside;

const GLSLANG_MODULE: &str = "slang-glslang.dll";

const SLANG_MODULES: [&str; 2] = ["slang-compiler.dll", "slang.dll"];

const SPIRV_TOOLS: &str = "SPIRV-Tools-shared.dll";

fn resolve(library: &Library) -> Option<SpirvTools> {
    resolve_tools!(library)
}

fn kept(library: Library) -> Option<SpirvTools> {
    let tools = resolve(&library)?;
    std::mem::forget(library);
    Some(tools)
}

pub(super) fn find() -> Result<SpirvTools, String> {
    let mut failures = Vec::new();
    match load_beside(GLSLANG_MODULE) {
        Ok(library) => {
            if let Some(tools) = kept(library) {
                return Ok(tools);
            }
        }
        Err(error) => failures.push(format!("{GLSLANG_MODULE}: {error}")),
    }
    let loaded = SLANG_MODULES
        .iter()
        .filter_map(|name| Library::open_already_loaded(name).ok())
        .find_map(kept);
    if let Some(tools) = loaded {
        return Ok(tools);
    }
    match load_beside(SPIRV_TOOLS) {
        Ok(library) => match kept(library) {
            Some(tools) => return Ok(tools),
            None => failures.push(format!("{SPIRV_TOOLS} lacks the optimizer C API")),
        },
        Err(error) => failures.push(format!("{SPIRV_TOOLS}: {error}")),
    }
    Err(format!(
        "no library exports the SPIRV-Tools optimizer ({})",
        failures.join("; ")
    ))
}
