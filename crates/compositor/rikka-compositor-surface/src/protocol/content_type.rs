use rikka_compositor_protocols::wp_content_type_manager_v1::{self, WpContentTypeManagerV1};
use rikka_compositor_protocols::wp_content_type_v1::{self, WpContentTypeV1};
use rikka_compositor_server::Client;
use rikka_compositor_types::ContentType;

use super::extension::{Claim, claim, target};
use crate::hints::content_type as hinted;
use crate::host::SurfaceHost;

pub(crate) fn manager<S: SurfaceHost>(
    state: &mut S,
    manager: WpContentTypeManagerV1,
    request: wp_content_type_manager_v1::Request,
) {
    let wp_content_type_manager_v1::Request::GetSurfaceContentType { id, surface } = request else {
        return;
    };
    match claim(state, &surface, |extensions| {
        !std::mem::replace(&mut extensions.content_type, true)
    }) {
        Claim::Claimed(surface) => id.set_key(surface),
        Claim::Occupied => {
            manager.post_error(wp_content_type_manager_v1::Error::AlreadyConstructed)
        }
        Claim::Missing => {}
    }
}

pub(crate) fn request<S: SurfaceHost>(
    state: &mut S,
    object: WpContentTypeV1,
    request: wp_content_type_v1::Request,
) {
    let wp_content_type_v1::Request::SetContentType { content_type } = request else {
        return;
    };
    let Ok(content_type) = content_type else {
        if let Some(client) = Client::from_resource(&object) {
            client.kill("unknown wp_content_type_v1 content type");
        }
        return;
    };
    if let Some(surface) = target(state, &object) {
        surface.pending.update.content_type = Some(hinted(content_type));
    }
}

pub(crate) fn destroyed<S: SurfaceHost>(state: &mut S, object: WpContentTypeV1) {
    if let Some(surface) = target(state, &object) {
        surface.extensions.content_type = false;
        surface.pending.update.content_type = Some(ContentType::None);
    }
}
