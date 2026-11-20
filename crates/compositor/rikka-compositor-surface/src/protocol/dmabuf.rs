use rikka_compositor_protocols::wl_buffer::WlBuffer;
use rikka_compositor_protocols::zwp_linux_buffer_params_v1::{self, ZwpLinuxBufferParamsV1};
use rikka_compositor_protocols::zwp_linux_dmabuf_feedback_v1::{self, ZwpLinuxDmabufFeedbackV1};
use rikka_compositor_protocols::zwp_linux_dmabuf_v1::{self, ZwpLinuxDmabufV1};
use rikka_compositor_server::Client;
use rikka_compositor_types::Format;

use crate::buffer::Storage;
use crate::dmabuf::{Dmabuf, DmabufFlags, Params};
use crate::host::SurfaceHost;
use crate::ids::ParamsId;

const FEEDBACK_SINCE: u32 = 4;
const MODIFIER_EVENT_SINCE: u32 = 3;

pub(crate) fn bound<S: SurfaceHost>(state: &mut S, dmabuf: ZwpLinuxDmabufV1) {
    if dmabuf.version() >= FEEDBACK_SINCE {
        return;
    }
    let Some(feedback) = state.surfaces().dmabuf.default.clone() else {
        return;
    };
    let mut formats: Vec<Format> = feedback.formats().copied().collect();
    formats.sort();
    let mut codes: Vec<u32> = formats.iter().map(|format| format.fourcc.0).collect();
    codes.dedup();
    for code in codes {
        dmabuf.format(code);
    }
    if dmabuf.version() >= MODIFIER_EVENT_SINCE {
        for format in formats {
            let modifier = format.modifier.0;
            dmabuf.modifier(format.fourcc.0, (modifier >> 32) as u32, modifier as u32);
        }
    }
}

pub(crate) fn request<S: SurfaceHost>(
    state: &mut S,
    _dmabuf: ZwpLinuxDmabufV1,
    request: zwp_linux_dmabuf_v1::Request,
) {
    let surfaces = state.surfaces();
    match request {
        zwp_linux_dmabuf_v1::Request::Destroy => {}
        zwp_linux_dmabuf_v1::Request::CreateParams { params_id } => {
            params_id.set_key(surfaces.objects.params.insert(Params::default()));
        }
        zwp_linux_dmabuf_v1::Request::GetDefaultFeedback { id } => {
            if let Some(feedback) = surfaces.dmabuf_feedback_for(None) {
                feedback.send(&id);
            }
            surfaces.dmabuf.objects.push(id);
        }
        zwp_linux_dmabuf_v1::Request::GetSurfaceFeedback { id, surface } => {
            let surface = surfaces.lookup(&surface);
            if let Some(feedback) = surfaces.dmabuf_feedback_for(surface) {
                feedback.send(&id);
            }
            if let Some(target) = surface.and_then(|surface| surfaces.surfaces.get_mut(surface)) {
                target.extensions.dmabuf_feedback.push(id);
            }
        }
    }
}

pub(crate) fn feedback<S>(
    _state: &mut S,
    _feedback: ZwpLinuxDmabufFeedbackV1,
    _request: zwp_linux_dmabuf_feedback_v1::Request,
) {
}

pub(crate) fn params<S: SurfaceHost>(
    state: &mut S,
    params: ZwpLinuxBufferParamsV1,
    request: zwp_linux_buffer_params_v1::Request,
) {
    let Some(id) = params.key::<ParamsId>() else {
        return;
    };
    let outcome = match request {
        zwp_linux_buffer_params_v1::Request::Destroy => Ok(()),
        zwp_linux_buffer_params_v1::Request::Add {
            fd,
            plane_idx,
            offset,
            stride,
            modifier_hi,
            modifier_lo,
        } => {
            let modifier = (u64::from(modifier_hi) << 32) | u64::from(modifier_lo);
            let version = params.version();
            state
                .surfaces()
                .objects
                .params
                .get_mut(id)
                .map_or(Ok(()), |pending| {
                    pending.add(fd, plane_idx, offset, stride, modifier, version)
                })
        }
        zwp_linux_buffer_params_v1::Request::SetSamplingDevice { device } => state
            .surfaces()
            .objects
            .params
            .get_mut(id)
            .map_or(Ok(()), |pending| pending.set_sampling_device(device)),
        zwp_linux_buffer_params_v1::Request::Create {
            width,
            height,
            format,
            flags,
        } => build(state, id, width, height, format, flags).map(|built| {
            match built.and_then(|dmabuf| import(state, &params, dmabuf, None)) {
                Some(buffer) => params.created(&buffer),
                None => params.failed(),
            }
        }),
        zwp_linux_buffer_params_v1::Request::CreateImmed {
            buffer_id,
            width,
            height,
            format,
            flags,
        } => build(state, id, width, height, format, flags).map(|built| {
            if built
                .and_then(|dmabuf| import(state, &params, dmabuf, Some(buffer_id)))
                .is_none()
            {
                params.post_error(zwp_linux_buffer_params_v1::Error::InvalidWlBuffer);
            }
        }),
    };
    if let Err(invalid) = outcome {
        params.post_error(invalid.error());
    }
}

pub(crate) fn params_destroyed<S: SurfaceHost>(state: &mut S, params: ZwpLinuxBufferParamsV1) {
    if let Some(id) = params.key::<ParamsId>() {
        state.surfaces().objects.params.remove(id);
    }
}

fn build<S: SurfaceHost>(
    state: &mut S,
    id: ParamsId,
    width: i32,
    height: i32,
    format: u32,
    flags: DmabufFlags,
) -> Result<Option<Dmabuf>, crate::dmabuf::Invalid> {
    let surfaces = state.surfaces();
    let (dmabuf, pending) = (&surfaces.dmabuf, surfaces.objects.params.get_mut(id));
    let Some(pending) = pending else {
        return Ok(None);
    };
    pending
        .build(width, height, format, flags, |format| {
            dmabuf.supports(format)
        })
        .map(Some)
}

fn import<S: SurfaceHost>(
    state: &mut S,
    params: &ZwpLinuxBufferParamsV1,
    dmabuf: Dmabuf,
    immediate: Option<WlBuffer>,
) -> Option<WlBuffer> {
    if !state.import_dmabuf(&dmabuf) {
        return None;
    }
    let buffer = match immediate {
        Some(buffer) => buffer,
        None => Client::from_resource(params)?.create::<WlBuffer>(1),
    };
    let size = dmabuf.size;
    let id =
        state
            .surfaces()
            .buffers
            .create(buffer.clone(), Storage::Dmabuf(Box::new(dmabuf)), size);
    buffer.set_key(id);
    Some(buffer)
}
