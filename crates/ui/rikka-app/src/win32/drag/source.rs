use std::cell::Cell;
use std::rc::Rc;

use windows::Win32::Foundation::{
    DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, S_OK,
};
use windows::Win32::System::Ole::{DROPEFFECT, IDropSource, IDropSource_Impl};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::core::{BOOL, HRESULT, implement};

use crate::win32::active::with_runner;

#[implement(IDropSource)]
pub(crate) struct DropSource {
    escaped: Rc<Cell<bool>>,
}

impl DropSource {
    pub(crate) fn new(escaped: Rc<Cell<bool>>) -> DropSource {
        DropSource { escaped }
    }
}

impl IDropSource_Impl for DropSource_Impl {
    fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
        if escape.as_bool() {
            self.escaped.set(true);
            return DRAGDROP_S_CANCEL;
        }
        if keys.0 & MK_LBUTTON.0 == 0 {
            return DRAGDROP_S_DROP;
        }
        if with_runner(|runner| runner.drag_step()).is_none() {
            tracing::trace!("drag step skipped while the runner is busy");
        }
        S_OK
    }

    fn GiveFeedback(&self, _: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}
