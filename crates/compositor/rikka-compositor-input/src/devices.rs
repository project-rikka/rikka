use input::event::{Event, EventTrait};
use input::{AsRaw, Device, DeviceCapability, ffi};
use rikka_compositor_geometry::Size;

use crate::device::{Capabilities, DeviceId, DeviceInfo, PadGroup, PadLayout, PointerKind};

#[derive(Debug)]
pub(crate) struct TrackedDevice {
    pub(crate) device: Device,
    pub(crate) id: DeviceId,
    pub(crate) pointer: Option<PointerKind>,
    pub(crate) keyboard: bool,
    pub(crate) natural: bool,
}

impl TrackedDevice {
    pub(crate) fn refresh_natural_scroll(&mut self) {
        self.natural = self.device.config_scroll_natural_scroll_enabled();
    }
}

#[derive(Debug, Default)]
pub(crate) struct Devices {
    tracked: Vec<TrackedDevice>,
    next: u64,
}

impl Devices {
    pub(crate) fn add(&mut self, device: Device) -> &mut TrackedDevice {
        let id = DeviceId::new(self.next);
        self.next += 1;
        self.tracked.push(TrackedDevice {
            pointer: pointer_kind(&device),
            keyboard: device.has_capability(DeviceCapability::Keyboard),
            natural: device.config_scroll_natural_scroll_enabled(),
            device,
            id,
        });
        let last = self.tracked.len() - 1;
        &mut self.tracked[last]
    }

    pub(crate) fn remove(&mut self, device: &Device) -> Option<DeviceId> {
        let index = self
            .tracked
            .iter()
            .position(|tracked| tracked.device == *device)?;
        Some(self.tracked.swap_remove(index).id)
    }

    pub(crate) fn for_event(&self, event: &Event) -> Option<&TrackedDevice> {
        let device = unsafe { ffi::libinput_event_get_device(event.as_raw_event()) };
        self.tracked
            .iter()
            .find(|tracked| tracked.device.as_raw() == device.cast_const())
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut TrackedDevice> {
        self.tracked.iter_mut()
    }
}

fn pointer_kind(device: &Device) -> Option<PointerKind> {
    if !device.has_capability(DeviceCapability::Pointer) {
        return None;
    }
    Some(if device.config_tap_finger_count() > 0 {
        PointerKind::Touchpad
    } else {
        PointerKind::Mouse
    })
}

pub(crate) fn describe(tracked: &mut TrackedDevice) -> DeviceInfo {
    let device = &mut tracked.device;
    DeviceInfo {
        id: tracked.id,
        name: device.name().to_owned(),
        sysname: device.sysname().to_owned(),
        vendor: device.id_vendor(),
        product: device.id_product(),
        capabilities: capabilities(device),
        pointer: tracked.pointer,
        size: device
            .size()
            .map(|(width, height)| Size::new(width, height)),
        output: device.output_name().map(str::to_owned),
        touch_slots: device
            .has_capability(DeviceCapability::Touch)
            .then(|| device.touch_count())
            .flatten(),
        pad: device
            .has_capability(DeviceCapability::TabletPad)
            .then(|| pad_layout(device)),
    }
}

fn capabilities(device: &Device) -> Capabilities {
    [
        (DeviceCapability::Keyboard, Capabilities::KEYBOARD),
        (DeviceCapability::Pointer, Capabilities::POINTER),
        (DeviceCapability::Touch, Capabilities::TOUCH),
        (DeviceCapability::TabletTool, Capabilities::TABLET_TOOL),
        (DeviceCapability::TabletPad, Capabilities::TABLET_PAD),
        (DeviceCapability::Gesture, Capabilities::GESTURE),
        (DeviceCapability::Switch, Capabilities::SWITCH),
    ]
    .into_iter()
    .filter(|(capability, _)| device.has_capability(*capability))
    .fold(Capabilities::empty(), |all, (_, flag)| all | flag)
}

fn pad_layout(device: &Device) -> PadLayout {
    let count = |number: i32| u32::try_from(number).unwrap_or(0);
    let buttons = count(device.tablet_pad_number_of_buttons());
    let rings = count(device.tablet_pad_number_of_rings());
    let strips = count(device.tablet_pad_number_of_strips());
    let groups = (0..count(device.tablet_pad_number_of_mode_groups()))
        .filter_map(|index| device.tablet_pad_mode_group(index))
        .map(|group| PadGroup {
            index: group.index(),
            modes: group.number_of_modes(),
            mode: group.mode(),
            buttons: (0..buttons)
                .filter(|&button| group.has_button(button))
                .collect(),
            toggles: (0..buttons)
                .filter(|&button| group.button_is_toggle(button))
                .collect(),
            rings: (0..rings).filter(|&ring| group.has_ring(ring)).collect(),
            strips: (0..strips)
                .filter(|&strip| group.has_strip(strip))
                .collect(),
        })
        .collect();
    PadLayout {
        buttons,
        rings,
        strips,
        groups,
    }
}
