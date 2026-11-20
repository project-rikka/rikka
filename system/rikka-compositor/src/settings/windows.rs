use rikka_compositor_planes::Tearing;
use rikka_compositor_tiling::Gaps;
use rikka_settings::Schema;
use rikka_settings::seam::Settings;
use rikka_settings::session::windows::{
    self, ALLOW_TEARING, ALLOW_TEARING_DEFAULT, FOCUS_OUTLINE, GAP_INNER, GAP_OUTER, LAYOUT,
    RESIZE_ON_BORDER, SCROLLING, SCROLLING_DEFAULT, SNAP, SNAP_ASSIST, SNAP_BAR, SNAP_TOGETHER,
};

use crate::policy::{LayoutMode, SnapRules, WindowRules};

pub(crate) use rikka_settings::session::windows::DOMAIN;

pub(crate) fn schema() -> Schema {
    windows::schema()
}

pub(crate) fn rules(settings: &Settings) -> WindowRules {
    let declared = WindowRules::declared();
    let scrolling = settings.read_or(SCROLLING, SCROLLING_DEFAULT);
    let layout = settings.read::<String>(LAYOUT).and_then(|name| {
        LayoutMode::from_name(&name, scrolling).or_else(|| {
            tracing::warn!("unknown layout in `{DOMAIN}/{LAYOUT}`: {name}");
            None
        })
    });
    WindowRules {
        mode: layout.unwrap_or(declared.mode),
        gaps: Gaps {
            inner: settings.read_or(GAP_INNER, declared.gaps.inner),
            outer: settings.read_or(GAP_OUTER, declared.gaps.outer),
        },
        resize_on_border: settings.read_or(RESIZE_ON_BORDER, declared.resize_on_border),
        focus_outline: settings.read_or(FOCUS_OUTLINE, declared.focus_outline),
        snap: SnapRules {
            windows: settings.read_or(SNAP, declared.snap.windows),
            bar: settings.read_or(SNAP_BAR, declared.snap.bar),
            assist: settings.read_or(SNAP_ASSIST, declared.snap.assist),
            resize_together: settings.read_or(SNAP_TOGETHER, declared.snap.resize_together),
        },
    }
}

pub(crate) fn tearing(settings: &Settings) -> Tearing {
    match settings.read_or(ALLOW_TEARING, ALLOW_TEARING_DEFAULT) {
        true => Tearing::Allowed,
        false => Tearing::Forbidden,
    }
}
