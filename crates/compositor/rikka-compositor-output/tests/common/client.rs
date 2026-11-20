use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;

use rikka_test_display::Client;
use rikka_wayland_client::{
    Proxy, Queue, ext_background_effect_manager_v1, ext_background_effect_surface_v1,
    ext_foreign_toplevel_handle_v1, ext_foreign_toplevel_image_capture_source_manager_v1,
    ext_foreign_toplevel_list_v1, ext_image_capture_source_v1,
    ext_image_copy_capture_cursor_session_v1, ext_image_copy_capture_frame_v1,
    ext_image_copy_capture_manager_v1, ext_image_copy_capture_session_v1,
    ext_output_image_capture_source_manager_v1, wl_buffer, wl_compositor, wl_output, wl_pointer,
    wl_region, wl_seat, wl_shm, wl_shm_pool, wl_subcompositor, wl_subsurface, wl_surface,
    wp_drm_lease_connector_v1, wp_drm_lease_device_v1, wp_drm_lease_request_v1, wp_drm_lease_v1,
    zxdg_output_manager_v1, zxdg_output_v1,
};

#[derive(Debug, Default)]
pub(crate) struct Received {
    pub(crate) events: Vec<(String, String)>,
    pub(crate) fds: Vec<(String, OwnedFd)>,
    pub(crate) toplevels: Vec<ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1>,
    pub(crate) connectors: Vec<wp_drm_lease_connector_v1::WpDrmLeaseConnectorV1>,
}

impl Received {
    fn note<P: Proxy>(&mut self, proxy: &P, event: &impl Debug) {
        self.note_text(proxy, format!("{event:?}"));
    }

    fn note_text<P: Proxy>(&mut self, proxy: &P, text: String) {
        self.events.push((label(proxy), text));
    }
}

pub(crate) fn label<P: Proxy>(proxy: &P) -> String {
    format!("{}@{}", P::INTERFACE.name, proxy.id())
}

fn on_ext_foreign_toplevel_list_v1(
    received: &mut Received,
    proxy: ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
    event: ext_foreign_toplevel_list_v1::Event,
) {
    if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = &event {
        received.toplevels.push(toplevel.clone());
    }
    received.note(&proxy, &event);
}

fn on_wp_drm_lease_device_v1(
    received: &mut Received,
    proxy: wp_drm_lease_device_v1::WpDrmLeaseDeviceV1,
    event: wp_drm_lease_device_v1::Event,
) {
    match event {
        wp_drm_lease_device_v1::Event::DrmFd { fd } => {
            received.fds.push((label(&proxy), fd));
            received.note_text(&proxy, "DrmFd".to_owned());
        }
        wp_drm_lease_device_v1::Event::Connector { id } => {
            received.note_text(&proxy, format!("Connector {}", label(&id)));
            received.connectors.push(id);
        }
        other => received.note(&proxy, &other),
    }
}

fn on_wp_drm_lease_v1(
    received: &mut Received,
    proxy: wp_drm_lease_v1::WpDrmLeaseV1,
    event: wp_drm_lease_v1::Event,
) {
    match event {
        wp_drm_lease_v1::Event::LeaseFd { leased_fd } => {
            received.fds.push((label(&proxy), leased_fd));
            received.note_text(&proxy, "LeaseFd".to_owned());
        }
        other => received.note(&proxy, &other),
    }
}

pub(crate) struct Wayland(Client<Received>);

impl Deref for Wayland {
    type Target = Client<Received>;

    fn deref(&self) -> &Client<Received> {
        &self.0
    }
}

impl DerefMut for Wayland {
    fn deref_mut(&mut self) -> &mut Client<Received> {
        &mut self.0
    }
}

fn listen(queue: &mut Queue<Received>) {
    queue.on::<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1>(
        on_ext_foreign_toplevel_list_v1,
    );
    queue.on::<wp_drm_lease_device_v1::WpDrmLeaseDeviceV1>(on_wp_drm_lease_device_v1);
    queue.on::<wp_drm_lease_v1::WpDrmLeaseV1>(on_wp_drm_lease_v1);
    queue.on::<wl_output::WlOutput>(|received: &mut Received, proxy, event| {
        received.note(&proxy, &event)
    });
    queue.on::<wl_seat::WlSeat>(|received: &mut Received, proxy, event| {
        received.note(&proxy, &event)
    });
    queue.on::<wl_pointer::WlPointer>(|received: &mut Received, proxy, event| {
        received.note(&proxy, &event)
    });
    queue.on::<wl_compositor::WlCompositor>(|received: &mut Received, proxy, event| {
        received.note(&proxy, &event)
    });
    queue.on::<wl_surface::WlSurface>(|received: &mut Received, proxy, event| {
        received.note(&proxy, &event)
    });
    queue.on::<wl_region::WlRegion>(|received: &mut Received, proxy, event| {
        received.note(&proxy, &event)
    });
    queue
        .on::<wl_shm::WlShm>(|received: &mut Received, proxy, event| received.note(&proxy, &event));
    queue.on::<wl_shm_pool::WlShmPool>(|received: &mut Received, proxy, event| {
        received.note(&proxy, &event)
    });
    queue.on::<wl_buffer::WlBuffer>(|received: &mut Received, proxy, event| {
        received.note(&proxy, &event)
    });
    queue.on::<wl_subcompositor::WlSubcompositor>(|received: &mut Received, proxy, event| {
        received.note(&proxy, &event)
    });
    queue.on::<wl_subsurface::WlSubsurface>(|received: &mut Received, proxy, event| {
        received.note(&proxy, &event)
    });
    queue.on::<zxdg_output_manager_v1::ZxdgOutputManagerV1>(
        |received: &mut Received, proxy, event| received.note(&proxy, &event),
    );
    queue.on::<zxdg_output_v1::ZxdgOutputV1>(|received: &mut Received, proxy, event| {
        received.note(&proxy, &event)
    });
    queue.on::<ext_image_capture_source_v1::ExtImageCaptureSourceV1>(
        |received: &mut Received, proxy, event| received.note(&proxy, &event),
    );
    queue.on::<ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1>(
        |received: &mut Received, proxy, event| received.note(&proxy, &event),
    );
    queue.on::<ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1>(|received: &mut Received, proxy, event| received.note(&proxy, &event));
    queue.on::<ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1>(
        |received: &mut Received, proxy, event| received.note(&proxy, &event),
    );
    queue.on::<ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1>(
        |received: &mut Received, proxy, event| received.note(&proxy, &event),
    );
    queue.on::<ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1>(
        |received: &mut Received, proxy, event| received.note(&proxy, &event),
    );
    queue.on::<ext_image_copy_capture_cursor_session_v1::ExtImageCopyCaptureCursorSessionV1>(
        |received: &mut Received, proxy, event| received.note(&proxy, &event),
    );
    queue.on::<ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1>(
        |received: &mut Received, proxy, event| received.note(&proxy, &event),
    );
    queue.on::<ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1>(
        |received: &mut Received, proxy, event| received.note(&proxy, &event),
    );
    queue.on::<ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1>(
        |received: &mut Received, proxy, event| received.note(&proxy, &event),
    );
    queue.on::<wp_drm_lease_connector_v1::WpDrmLeaseConnectorV1>(
        |received: &mut Received, proxy, event| received.note(&proxy, &event),
    );
    queue.on::<wp_drm_lease_request_v1::WpDrmLeaseRequestV1>(
        |received: &mut Received, proxy, event| received.note(&proxy, &event),
    );
}

impl Wayland {
    pub(crate) fn new(stream: UnixStream) -> Self {
        Self(Client::connect(stream, listen))
    }

    pub(crate) fn global_names(&self, interface: &str) -> Vec<u32> {
        self.globals()
            .into_iter()
            .filter(|global| &*global.interface == interface)
            .map(|global| global.name)
            .collect()
    }

    pub(crate) fn events<P: Proxy>(&self, proxy: &P) -> Vec<String> {
        let owner_label = label(proxy);
        self.received
            .events
            .iter()
            .filter(|(owner, _)| *owner == owner_label)
            .map(|(_, event)| event.clone())
            .collect()
    }

    pub(crate) fn forget_events(&mut self) {
        self.received.events.clear();
    }

    pub(crate) fn surface(&self) -> wl_surface::WlSurface {
        let compositor: wl_compositor::WlCompositor = self.global(6);
        compositor.create_surface()
    }

    pub(crate) fn region(&self, rects: &[(i32, i32, i32, i32)]) -> wl_region::WlRegion {
        let compositor: wl_compositor::WlCompositor = self.global(6);
        let region = compositor.create_region();
        for (x, y, width, height) in rects {
            region.add(*x, *y, *width, *height);
        }
        region
    }
}
