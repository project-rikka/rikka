use rikka_wayland_client::wp_color_representation_manager_v1;
use rikka_wayland_client::{
    wp_color_management_output_v1, wp_color_management_surface_feedback_v1, wp_color_manager_v1,
    wp_image_description_info_v1, wp_image_description_v1,
};
use rikka_wayland_client::{
    zwp_linux_buffer_params_v1, zwp_linux_dmabuf_feedback_v1, zwp_linux_dmabuf_v1,
};

use rikka_wayland_client::{Queue, raw_value};

use super::Received;

fn on_zwp_linux_dmabuf_v1(
    received: &mut Received,
    _: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
    event: zwp_linux_dmabuf_v1::Event,
) {
    match event {
        zwp_linux_dmabuf_v1::Event::Format { format } => {
            received.dmabuf.push(format!("format {format:#x}"));
        }
        zwp_linux_dmabuf_v1::Event::Modifier {
            format,
            modifier_hi,
            modifier_lo,
        } => received
            .dmabuf
            .push(format!("modifier {format:#x} {modifier_hi} {modifier_lo}")),
        _ => {}
    }
}

fn on_zwp_linux_dmabuf_feedback_v1(
    received: &mut Received,
    _: zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1,
    event: zwp_linux_dmabuf_feedback_v1::Event,
) {
    use zwp_linux_dmabuf_feedback_v1::Event;
    let line = match event {
        Event::FormatTable { fd, size } => {
            received.tables.push((fd, size));
            format!("table {size}")
        }
        Event::MainDevice { device } => format!("main {}", device_of(&device)),
        Event::TrancheTargetDevice { device } => format!("target {}", device_of(&device)),
        Event::TrancheFormats { indices } => {
            let indices: Vec<u16> = indices
                .chunks(2)
                .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
                .collect();
            format!("formats {indices:?}")
        }
        Event::TrancheFlags { flags } => format!("flags {}", flags.bits()),
        Event::TrancheDone => "tranche done".to_owned(),
        Event::Done => "done".to_owned(),
        _ => return,
    };
    received.dmabuf.push(line);
}

fn device_of(bytes: &[u8]) -> u64 {
    u64::from_ne_bytes(bytes.try_into().unwrap())
}

fn on_zwp_linux_buffer_params_v1(
    received: &mut Received,
    _: zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
    event: zwp_linux_buffer_params_v1::Event,
) {
    match event {
        zwp_linux_buffer_params_v1::Event::Created { buffer } => received.created.push(buffer),
        zwp_linux_buffer_params_v1::Event::Failed => received.dmabuf.push("failed".to_owned()),
        _ => {}
    }
}

fn on_wp_color_manager_v1(
    received: &mut Received,
    _: wp_color_manager_v1::WpColorManagerV1,
    event: wp_color_manager_v1::Event,
) {
    use wp_color_manager_v1::Event;
    let line = match event {
        Event::SupportedIntent { render_intent } => format!("intent {}", raw_value(render_intent)),
        Event::SupportedFeature { feature } => format!("feature {}", raw_value(feature)),
        Event::SupportedTfNamed { tf } => format!("tf {}", raw_value(tf)),
        Event::SupportedPrimariesNamed { primaries } => {
            format!("primaries {}", raw_value(primaries))
        }
        Event::Done => "done".to_owned(),
        _ => return,
    };
    received.color.push(line);
}

fn on_wp_color_management_output_v1(
    received: &mut Received,
    _: wp_color_management_output_v1::WpColorManagementOutputV1,
    event: wp_color_management_output_v1::Event,
) {
    if let wp_color_management_output_v1::Event::ImageDescriptionChanged = event {
        received.color.push("output changed".to_owned());
    }
}

fn on_wp_color_management_surface_feedback_v1(
    received: &mut Received,
    _: wp_color_management_surface_feedback_v1::WpColorManagementSurfaceFeedbackV1,
    event: wp_color_management_surface_feedback_v1::Event,
) {
    if let wp_color_management_surface_feedback_v1::Event::PreferredChanged2 {
        identity_hi,
        identity_lo,
    } = event
    {
        let identity = (u64::from(identity_hi) << 32) | u64::from(identity_lo);
        received.color.push(format!("preferred {identity}"));
    }
}

fn on_wp_image_description_v1(
    received: &mut Received,
    proxy: wp_image_description_v1::WpImageDescriptionV1,
    event: wp_image_description_v1::Event,
) {
    let tag = &proxy.key::<u32>().expect("the object is tagged");
    let line = match event {
        wp_image_description_v1::Event::Ready2 {
            identity_hi,
            identity_lo,
        } => format!(
            "ready {tag} {}",
            (u64::from(identity_hi) << 32) | u64::from(identity_lo)
        ),
        wp_image_description_v1::Event::Failed { cause, .. } => {
            format!("failed {tag} {}", raw_value(cause))
        }
        _ => return,
    };
    received.color.push(line);
}

fn on_wp_image_description_info_v1(
    received: &mut Received,
    _: wp_image_description_info_v1::WpImageDescriptionInfoV1,
    event: wp_image_description_info_v1::Event,
) {
    use wp_image_description_info_v1::Event;
    let line = match event {
        Event::Primaries {
            r_x, r_y, w_x, w_y, ..
        } => {
            format!("info primaries {r_x} {r_y} {w_x} {w_y}")
        }
        Event::PrimariesNamed { primaries } => {
            format!("info primaries named {}", raw_value(primaries))
        }
        Event::TfNamed { tf } => format!("info tf {}", raw_value(tf)),
        Event::TfPower { eexp } => format!("info power {eexp}"),
        Event::Luminances {
            min_lum,
            max_lum,
            reference_lum,
        } => format!("info luminances {min_lum} {max_lum} {reference_lum}"),
        Event::TargetPrimaries { .. } => "info target primaries".to_owned(),
        Event::TargetLuminance { min_lum, max_lum } => {
            format!("info target luminance {min_lum} {max_lum}")
        }
        Event::Done => "info done".to_owned(),
        _ => return,
    };
    received.color.push(line);
}

fn on_wp_color_representation_manager_v1(
    received: &mut Received,
    _: wp_color_representation_manager_v1::WpColorRepresentationManagerV1,
    event: wp_color_representation_manager_v1::Event,
) {
    use wp_color_representation_manager_v1::Event;
    let line = match event {
        Event::SupportedAlphaMode { alpha_mode } => format!("alpha {}", raw_value(alpha_mode)),
        Event::SupportedCoefficientsAndRanges {
            coefficients,
            range,
        } => {
            format!(
                "coefficients {} {}",
                raw_value(coefficients),
                raw_value(range)
            )
        }
        Event::Done => "representation done".to_owned(),
        _ => return,
    };
    received.color.push(line);
}

pub(super) fn listen(queue: &mut Queue<Received>) {
    queue.on::<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1>(on_zwp_linux_dmabuf_v1);
    queue.on::<zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1>(
        on_zwp_linux_dmabuf_feedback_v1,
    );
    queue.on::<zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1>(on_zwp_linux_buffer_params_v1);
    queue.on::<wp_color_manager_v1::WpColorManagerV1>(on_wp_color_manager_v1);
    queue.on::<wp_color_management_output_v1::WpColorManagementOutputV1>(
        on_wp_color_management_output_v1,
    );
    queue.on::<wp_color_management_surface_feedback_v1::WpColorManagementSurfaceFeedbackV1>(
        on_wp_color_management_surface_feedback_v1,
    );
    queue.on::<wp_image_description_v1::WpImageDescriptionV1>(on_wp_image_description_v1);
    queue.on::<wp_image_description_info_v1::WpImageDescriptionInfoV1>(
        on_wp_image_description_info_v1,
    );
    queue.on::<wp_color_representation_manager_v1::WpColorRepresentationManagerV1>(
        on_wp_color_representation_manager_v1,
    );
}
