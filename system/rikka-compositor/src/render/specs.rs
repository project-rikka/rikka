use std::os::fd::AsFd;

use rikka_compositor_kms::Dmabuf as KmsDmabuf;
use rikka_compositor_surface::Dmabuf;
use rikka_gpu::DmabufPlane;
use rikka_renderer::DmabufSpec;

pub(crate) fn with_client_spec<R>(dmabuf: &Dmabuf, read: impl FnOnce(&DmabufSpec<'_>) -> R) -> R {
    let planes: Vec<DmabufPlane<'_>> = dmabuf
        .planes()
        .iter()
        .map(|plane| DmabufPlane {
            offset: plane.offset(),
            stride: plane.stride(),
            fd: plane.fd(),
        })
        .collect();
    let size = dmabuf.size();
    let format = dmabuf.format();
    read(&DmabufSpec {
        width: size.w.max(0).unsigned_abs(),
        height: size.h.max(0).unsigned_abs(),
        fourcc: format.fourcc.0,
        modifier: format.modifier.0,
        planes: &planes,
    })
}

pub(crate) fn with_kms_spec<R>(dmabuf: &KmsDmabuf, read: impl FnOnce(&DmabufSpec<'_>) -> R) -> R {
    let planes: Vec<DmabufPlane<'_>> = dmabuf
        .planes()
        .iter()
        .map(|plane| DmabufPlane {
            offset: plane.offset,
            stride: plane.stride,
            fd: plane.fd.as_fd(),
        })
        .collect();
    let format = dmabuf.format();
    read(&DmabufSpec {
        width: dmabuf.width(),
        height: dmabuf.height(),
        fourcc: format.fourcc.0,
        modifier: format.modifier.0,
        planes: &planes,
    })
}
