mod schedules;

mod support {
    mod device;
    pub(crate) mod sample;
    mod scene;

    pub(crate) use device::cloth;
    pub(crate) use scene::{sheet_asset, style};
}
