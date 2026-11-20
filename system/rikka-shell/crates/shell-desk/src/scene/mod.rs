mod arrangement;
mod seat;
mod touch;
mod transition_style;
mod transitions;

use desk_layout::{Area, Layout, WidgetId};
use rikka_types::Size;
use rikka_ui::{Element, View};
use shell_kit::Placement;

use crate::desk_scene::DeskScene;
use crate::scene::arrangement::Arrangement;
use crate::scene::touch::Touch;
use crate::widgets::Mode;

#[derive(Debug)]
pub struct Scene {
    pub wallpaper: Option<String>,
    pub layout: Layout,
    pub output: Size,
    pub area: Area,
    pub mode: Mode,
    pub grabbed: Option<WidgetId>,
}

#[must_use]
pub fn scene(scene: Scene) -> Element {
    Arrangement { scene, touch: None }.build()
}

#[must_use]
pub(crate) fn desktop(scene: Scene, desk_scene: &DeskScene, screen: &Placement) -> Element {
    let touch = (scene.mode == Mode::Live).then(|| Touch {
        desk_scene: desk_scene.clone(),
        screen: screen.clone(),
    });
    Arrangement { scene, touch }.build()
}
