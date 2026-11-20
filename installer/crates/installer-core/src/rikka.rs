mod apps;
mod boot;
mod desktop;
mod ime;
mod integration;
mod module;
mod nix;
mod packages;
mod system;

pub use ime::fcitx5_addons;
pub(crate) use module::{RenderModule, section};
pub use packages::BASE_PACKAGES;
