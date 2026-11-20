mod egl;
mod vulkan;

use std::ffi::OsStr;
use std::path::Path;

pub(crate) use vulkan::SELECT_ENV as VULKAN_SELECT_ENV;

const DRM: &str = "/sys/class/drm";

const RENDER_NODE: &str = "renderD";

#[derive(Clone, Debug, PartialEq, Eq)]
struct RenderNode {
    driver: String,
}

pub unsafe fn narrow() {
    let Some(nodes) = render_nodes() else {
        return;
    };
    unsafe {
        egl::narrow(&nodes);
        vulkan::narrow(&nodes);
    }
}

fn render_nodes() -> Option<Vec<RenderNode>> {
    let mut nodes = Vec::new();
    for entry in std::fs::read_dir(DRM).ok()?.flatten() {
        let path = entry.path();
        if !path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.starts_with(RENDER_NODE))
        {
            continue;
        }
        nodes.push(described(&path.join("device"))?);
    }
    Some(nodes)
}

fn described(device: &Path) -> Option<RenderNode> {
    let driver = std::fs::read_link(device.join("driver")).ok()?;
    let driver = driver.file_name()?.to_str()?.to_owned();
    Some(RenderNode { driver })
}
