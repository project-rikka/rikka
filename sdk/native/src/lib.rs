mod abi;
mod api;
mod context;
mod error;
#[allow(dead_code, clippy::too_many_arguments)]
mod ffi;
mod handlers;
mod input;
mod media;
mod values;
mod view;
mod window;

rikka_app::mimalloc!();
