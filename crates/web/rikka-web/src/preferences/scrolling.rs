use rikka_types::SCROLL_LINE;
use rikka_ui::Curve;
use rikka_ui::motion::SCROLL;

use crate::preferences::list::{Pref, Preferences};

impl Preferences {
    pub(crate) fn with_toolkit_scrolling(self) -> Preferences {
        let Curve::Spring(spring) = SCROLL else {
            return self;
        };
        self.all(vec![
            ("general.smoothScroll.rikka.enabled", Pref::Bool(true)),
            (
                "general.smoothScroll.rikka.stiffness",
                Pref::Text(spring.stiffness.to_string()),
            ),
            (
                "general.smoothScroll.rikka.notch",
                Pref::Text(SCROLL_LINE.to_string()),
            ),
        ])
    }
}
