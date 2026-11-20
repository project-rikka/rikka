use browser_translate::Adapter;

use crate::google::Google;
use crate::microsoft::Microsoft;

pub(crate) static ALL: &[&dyn Adapter] = &[&Google, &Microsoft];
