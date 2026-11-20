use rikka_compositor_protocols::ext_idle_notification_v1::ExtIdleNotificationV1;
use rikka_compositor_protocols::ext_idle_notifier_v1::ExtIdleNotifierV1;
use rikka_compositor_protocols::wl_keyboard::WlKeyboard;
use rikka_compositor_protocols::wl_pointer::WlPointer;
use rikka_compositor_protocols::wl_seat::WlSeat;
use rikka_compositor_protocols::wl_touch::WlTouch;
use rikka_compositor_protocols::wp_cursor_shape_device_v1::WpCursorShapeDeviceV1;
use rikka_compositor_protocols::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;
use rikka_compositor_protocols::wp_pointer_warp_v1::WpPointerWarpV1;
use rikka_compositor_protocols::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;
use rikka_compositor_protocols::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1;
use rikka_compositor_protocols::zwp_confined_pointer_v1::ZwpConfinedPointerV1;
use rikka_compositor_protocols::zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1;
use rikka_compositor_protocols::zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1;
use rikka_compositor_protocols::zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2;
use rikka_compositor_protocols::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use rikka_compositor_protocols::zwp_input_method_v2::ZwpInputMethodV2;
use rikka_compositor_protocols::zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2;
use rikka_compositor_protocols::zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1;
use rikka_compositor_protocols::zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1;
use rikka_compositor_protocols::zwp_locked_pointer_v1::ZwpLockedPointerV1;
use rikka_compositor_protocols::zwp_pointer_constraints_v1::ZwpPointerConstraintsV1;
use rikka_compositor_protocols::zwp_pointer_gesture_hold_v1::ZwpPointerGestureHoldV1;
use rikka_compositor_protocols::zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1;
use rikka_compositor_protocols::zwp_pointer_gesture_swipe_v1::ZwpPointerGestureSwipeV1;
use rikka_compositor_protocols::zwp_pointer_gestures_v1::ZwpPointerGesturesV1;
use rikka_compositor_protocols::zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1;
use rikka_compositor_protocols::zwp_relative_pointer_v1::ZwpRelativePointerV1;
use rikka_compositor_protocols::zwp_tablet_manager_v2::ZwpTabletManagerV2;
use rikka_compositor_protocols::zwp_tablet_pad_dial_v2::ZwpTabletPadDialV2;
use rikka_compositor_protocols::zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2;
use rikka_compositor_protocols::zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2;
use rikka_compositor_protocols::zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2;
use rikka_compositor_protocols::zwp_tablet_pad_v2::ZwpTabletPadV2;
use rikka_compositor_protocols::zwp_tablet_seat_v2::ZwpTabletSeatV2;
use rikka_compositor_protocols::zwp_tablet_tool_v2::ZwpTabletToolV2;
use rikka_compositor_protocols::zwp_tablet_v2::ZwpTabletV2;
use rikka_compositor_protocols::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use rikka_compositor_protocols::zwp_text_input_v3::ZwpTextInputV3;
use rikka_compositor_protocols::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use rikka_compositor_protocols::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;
use rikka_compositor_protocols::zwp_xwayland_keyboard_grab_manager_v1::ZwpXwaylandKeyboardGrabManagerV1;
use rikka_compositor_protocols::zwp_xwayland_keyboard_grab_v1::ZwpXwaylandKeyboardGrabV1;
use rikka_compositor_server::{Client, DisplayHandle, Module};
use rikka_wayland_wire::Resource;

use crate::host::SeatHost;
use crate::pointer::ConstraintHandle;
use crate::{
    idle, ime, keyboard, pointer, tablet, touch, virtual_keyboard, virtual_pointer, wl_seat,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct SeatProtocols;

impl<S: SeatHost> Module<S> for SeatProtocols {
    fn install(self, display: &DisplayHandle<S>) {
        serve_devices(display);
        serve_pointer_extensions(display);
        serve_tablets(display);
        serve_text(display);
        serve_idle(display);

        display.global::<ZwpRelativePointerManagerV1>().publish();
        display.global::<ZwpPointerConstraintsV1>().publish();
        display.global::<ZwpPointerGesturesV1>().publish();
        display.global::<WpCursorShapeManagerV1>().publish();
        display.global::<WpPointerWarpV1>().publish();
        display
            .global::<ZwpKeyboardShortcutsInhibitManagerV1>()
            .publish();
        display.global::<ZwpTabletManagerV2>().publish();
        display.global::<ZwpTextInputManagerV3>().publish();
        display.global::<ZwpIdleInhibitManagerV1>().publish();
        display.global::<ExtIdleNotifierV1>().publish();
        display
            .global::<ZwpInputMethodManagerV2>()
            .privileged()
            .publish();
        display
            .global::<ZwpVirtualKeyboardManagerV1>()
            .privileged()
            .publish();
        display
            .global::<ZwlrVirtualPointerManagerV1>()
            .privileged()
            .publish();
        display
            .global::<ZwpXwaylandKeyboardGrabManagerV1>()
            .only(Client::is_xwayland)
            .publish();
    }
}

fn serve_devices<S: SeatHost>(display: &DisplayHandle<S>) {
    display
        .serve::<WlSeat>(wl_seat::seat_request)
        .bound(wl_seat::seat_bound)
        .destroyed(wl_seat::seat_destroyed);
    display
        .serve::<WlKeyboard>(keyboard::keyboard_request)
        .destroyed(keyboard::keyboard_destroyed);
    display
        .serve::<WlPointer>(pointer::pointer_request)
        .destroyed(pointer::pointer_destroyed);
    display
        .serve::<WlTouch>(touch::touch_request)
        .destroyed(touch::touch_destroyed);
    display.serve::<ZwpKeyboardShortcutsInhibitManagerV1>(keyboard::inhibit_manager);
    display
        .serve::<ZwpKeyboardShortcutsInhibitorV1>(keyboard::inhibitor)
        .destroyed(keyboard::inhibitor_destroyed);
    display.serve::<ZwpXwaylandKeyboardGrabManagerV1>(keyboard::xwayland_manager);
    display
        .serve::<ZwpXwaylandKeyboardGrabV1>(keyboard::xwayland_grab)
        .destroyed(keyboard::xwayland_grab_destroyed);
    display.serve::<ZwpVirtualKeyboardManagerV1>(virtual_keyboard::virtual_keyboard_manager);
    display
        .serve::<ZwpVirtualKeyboardV1>(virtual_keyboard::virtual_keyboard_request)
        .destroyed(virtual_keyboard::virtual_keyboard_destroyed);
    display.serve::<ZwlrVirtualPointerManagerV1>(virtual_pointer::virtual_pointer_manager);
    display
        .serve::<ZwlrVirtualPointerV1>(virtual_pointer::virtual_pointer_request)
        .destroyed(virtual_pointer::virtual_pointer_destroyed);
}

fn serve_pointer_extensions<S: SeatHost>(display: &DisplayHandle<S>) {
    display.serve::<ZwpRelativePointerManagerV1>(pointer::relative_manager);
    display
        .serve::<ZwpRelativePointerV1>(pointer::relative_request)
        .destroyed(pointer::relative_destroyed);
    display.serve::<ZwpPointerConstraintsV1>(pointer::constraints_manager);
    display
        .serve::<ZwpLockedPointerV1>(pointer::locked_request)
        .destroyed(|state: &mut S, lock| {
            pointer::constraint_destroyed(state, ConstraintHandle::Lock(lock))
        });
    display
        .serve::<ZwpConfinedPointerV1>(pointer::confined_request)
        .destroyed(|state: &mut S, confine| {
            pointer::constraint_destroyed(state, ConstraintHandle::Confine(confine));
        });
    display.serve::<ZwpPointerGesturesV1>(pointer::gestures_manager);
    gesture::<S, ZwpPointerGestureSwipeV1>(display);
    gesture::<S, ZwpPointerGesturePinchV1>(display);
    gesture::<S, ZwpPointerGestureHoldV1>(display);
    display.serve::<WpCursorShapeManagerV1>(pointer::shape_manager);
    display.serve::<WpCursorShapeDeviceV1>(pointer::shape_device);
    display.serve::<WpPointerWarpV1>(pointer::warp_request);
}

fn gesture<S: SeatHost, R: Resource>(display: &DisplayHandle<S>) {
    display
        .serve::<R>(pointer::gesture_request::<S, R>)
        .destroyed(pointer::gesture_destroyed::<S, R>);
}

fn serve_tablets<S: SeatHost>(display: &DisplayHandle<S>) {
    display.serve::<ZwpTabletManagerV2>(tablet::tablet_manager);
    display
        .serve::<ZwpTabletSeatV2>(tablet::tablet_seat_request)
        .destroyed(tablet::tablet_seat_destroyed);
    display
        .serve::<ZwpTabletV2>(tablet::tablet_request)
        .destroyed(tablet::tablet_destroyed);
    display
        .serve::<ZwpTabletToolV2>(tablet::tool_request)
        .destroyed(tablet::tool_destroyed);
    pad_part::<S, ZwpTabletPadV2>(display);
    pad_part::<S, ZwpTabletPadGroupV2>(display);
    pad_part::<S, ZwpTabletPadRingV2>(display);
    pad_part::<S, ZwpTabletPadStripV2>(display);
    pad_part::<S, ZwpTabletPadDialV2>(display);
}

fn pad_part<S: SeatHost, R: Resource>(display: &DisplayHandle<S>) {
    display
        .serve::<R>(tablet::pad_part_request::<S, R>)
        .destroyed(tablet::pad_part_destroyed::<S, R>);
}

fn serve_text<S: SeatHost>(display: &DisplayHandle<S>) {
    display.serve::<ZwpTextInputManagerV3>(ime::text_input_manager);
    display
        .serve::<ZwpTextInputV3>(ime::text_input_request)
        .destroyed(ime::text_input_destroyed);
    display.serve::<ZwpInputMethodManagerV2>(ime::input_method_manager);
    display
        .serve::<ZwpInputMethodV2>(ime::input_method_request)
        .destroyed(ime::input_method_destroyed);
    display
        .serve::<ZwpInputMethodKeyboardGrabV2>(ime::grab_request)
        .destroyed(ime::grab_destroyed);
    display
        .serve::<ZwpInputPopupSurfaceV2>(ime::popup_request)
        .destroyed(ime::popup_destroyed);
}

fn serve_idle<S: SeatHost>(display: &DisplayHandle<S>) {
    display.serve::<ZwpIdleInhibitManagerV1>(idle::idle_inhibit_manager);
    display
        .serve::<ZwpIdleInhibitorV1>(idle::idle_inhibitor)
        .destroyed(idle::idle_inhibitor_destroyed);
    display.serve::<ExtIdleNotifierV1>(idle::idle_notifier);
    display
        .serve::<ExtIdleNotificationV1>(idle::idle_notification)
        .destroyed(idle::idle_notification_destroyed);
}
