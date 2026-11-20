use rikka_compositor_protocols::wp_cursor_shape_device_v1::{
    self as device, WpCursorShapeDeviceV1,
};
use rikka_compositor_protocols::wp_cursor_shape_manager_v1::{
    self as manager, WpCursorShapeManagerV1,
};

use crate::access::Access;
use crate::ids::{SeatId, ToolId};
use crate::pointer::cursor::{CursorImage, CursorOwner, may_set_cursor, show};

pub(crate) fn shape_manager<A: Access>(
    _state: &mut A,
    _manager: WpCursorShapeManagerV1,
    request: manager::Request,
) {
    match request {
        manager::Request::Destroy => {}
        manager::Request::GetPointer {
            cursor_shape_device,
            pointer,
        } => {
            if let Some(seat) = pointer.key::<SeatId>() {
                cursor_shape_device.set_key(seat);
            }
        }
        manager::Request::GetTabletToolV2 {
            cursor_shape_device,
            tablet_tool,
        } => {
            if let (Some(seat), Some(tool)) =
                (tablet_tool.key::<SeatId>(), tablet_tool.key::<ToolId>())
            {
                cursor_shape_device.set_key(seat);
                cursor_shape_device.set_key(tool);
            }
        }
    }
}

pub(crate) fn shape_device<A: Access>(
    state: &mut A,
    device: WpCursorShapeDeviceV1,
    request: device::Request,
) {
    let device::Request::SetShape { serial, shape } = request else {
        return;
    };
    let Ok(shape) = shape else {
        device.post_error(device::Error::InvalidShape);
        return;
    };
    let Some(seat) = device.key::<SeatId>() else {
        return;
    };
    let owner = match device.key::<ToolId>() {
        Some(tool) => CursorOwner::Tool(tool),
        None => CursorOwner::Pointer,
    };
    if !may_set_cursor(state, seat, owner, device.client(), serial) {
        return;
    }
    show(state, seat, owner, CursorImage::Shape(shape.into()), None);
}
