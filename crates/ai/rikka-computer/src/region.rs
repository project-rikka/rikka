#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

impl Region {
    #[must_use]
    pub fn from_corners(top_left: (u32, u32), bottom_right: (u32, u32)) -> Option<Region> {
        let ((left, top), (right, bottom)) = (top_left, bottom_right);
        (right > left && bottom > top).then_some(Region {
            left,
            top,
            right,
            bottom,
        })
    }

    pub(crate) fn fits(self, (width, height): (u32, u32)) -> bool {
        self.right <= width && self.bottom <= height
    }

    pub(crate) fn scaled(self, from: (u32, u32), to: (u32, u32)) -> rikka_image::Region {
        let across = |length: u32| scaled(length, from.0, to.0);
        let down = |length: u32| scaled(length, from.1, to.1);
        let (left, top) = (across(self.left), down(self.top));
        rikka_image::Region::new(
            left,
            top,
            across(self.right).saturating_sub(left).max(1),
            down(self.bottom).saturating_sub(top).max(1),
        )
    }
}

impl std::fmt::Display for Region {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "[{}, {}, {}, {}]",
            self.left, self.top, self.right, self.bottom
        )
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "maps a screenshot length onto the captured frame, both u32 sized"
)]
fn scaled(length: u32, from: u32, to: u32) -> u32 {
    (f64::from(length) * f64::from(to) / f64::from(from.max(1))).round() as u32
}
