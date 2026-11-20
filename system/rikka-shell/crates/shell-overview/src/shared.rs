use rikka_seam::SeamClient;
use shell_desk::DeskScene;
use shell_kit::DesktopFrost;

use crate::desktop::Desktop;
use crate::swipe::Swipes;

#[derive(Clone, Debug)]
pub(crate) struct Shared {
    pub(crate) seam: SeamClient,
    pub(crate) desktop: Desktop,
    pub(super) swipes: Swipes,
    pub(crate) desk_scene: DeskScene,
    pub(crate) frost: DesktopFrost,
}
