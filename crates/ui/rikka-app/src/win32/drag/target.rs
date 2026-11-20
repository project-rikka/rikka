use windows::Win32::Foundation::{HWND, POINTL};
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::Ole::{
    DROPEFFECT, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl, RegisterDragDrop, RevokeDragDrop,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::core::{Ref, implement};

use crate::tasks::WindowId;
use crate::win32::active::with_runner;

#[implement(IDropTarget)]
struct DropTarget {
    window: WindowId,
}

#[derive(Debug)]
pub(crate) struct DropRegistration(HWND);

impl Drop for DropRegistration {
    fn drop(&mut self) {
        if let Err(error) = unsafe { RevokeDragDrop(self.0) } {
            tracing::debug!("failed to stop accepting drops: {error}");
        }
    }
}

pub(crate) fn register(window: HWND, id: WindowId) -> Option<DropRegistration> {
    let target: IDropTarget = DropTarget { window: id }.into();
    match unsafe { RegisterDragDrop(window, &target) } {
        Ok(()) => Some(DropRegistration(window)),
        Err(error) => {
            tracing::warn!("window cannot accept drops: {error}");
            None
        }
    }
}

fn answer(effect: *mut DROPEFFECT, act: impl FnOnce(DROPEFFECT) -> Option<DROPEFFECT>) {
    let Some(effect) = (unsafe { effect.as_mut() }) else {
        return;
    };
    *effect = act(*effect).unwrap_or(DROPEFFECT_NONE);
}

fn screen(point: &POINTL) -> (i32, i32) {
    (point.x, point.y)
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<IDataObject>,
        _: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let data = data.ok()?.clone();
        answer(effect, |allowed| {
            with_runner(|runner| runner.drag_entered(self.window, data, screen(point), allowed))
        });
        Ok(())
    }

    fn DragOver(
        &self,
        _: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        answer(effect, |allowed| {
            with_runner(|runner| runner.drag_moved(self.window, screen(point), allowed))
        });
        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        with_runner(|runner| runner.drag_left(self.window));
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<IDataObject>,
        _: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let data = data.ok()?.clone();
        answer(effect, |allowed| {
            with_runner(|runner| runner.drag_dropped(self.window, data, screen(point), allowed))
        });
        Ok(())
    }
}
