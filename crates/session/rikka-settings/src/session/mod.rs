pub mod animation;
pub mod appearance;
pub mod capture;
pub mod clipboard;
pub mod color_picker;
pub mod displays;
pub mod emoji;
pub mod ime;
pub mod language;
pub mod lock;
pub mod network;
pub mod notifications;
pub mod pointer;
pub mod power;
pub mod remote;
pub mod replay;
pub mod translate;
pub mod weather;
pub mod windows;

use crate::catalog::Catalog;

#[must_use]
pub fn catalog() -> Catalog {
    [
        animation::schema(),
        appearance::schema(),
        capture::schema(),
        clipboard::schema(),
        color_picker::schema(),
        ime::schema(),
        language::schema(),
        lock::schema(),
        network::schema(),
        notifications::schema(),
        pointer::schema(),
        power::schema(),
        remote::schema(),
        replay::schema(),
        weather::schema(),
        windows::schema(),
    ]
    .into_iter()
    .fold(Catalog::new(), Catalog::schema)
    .instanced(displays::INSTANCES, displays::template())
}
