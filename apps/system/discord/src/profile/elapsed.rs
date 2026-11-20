use std::time::Duration;

use rikka_app::prelude::*;

const LINE: f32 = 16.0;
const SECOND: i64 = 1000;

pub(super) fn elapsed(started: i64) -> Element {
    Element::new(Elapsed { started })
}

struct Elapsed {
    started: i64,
}

impl Widget for Elapsed {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(LINE))
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let spent = (jiff::Timestamp::now().as_millisecond() - self.started).max(0);
        let clock = rikka_format::format_duration(Duration::from_millis(spent.unsigned_abs()));
        let caption = tr!("discord.profile.activity.elapsed", time = clock.as_str());
        let style = TextStyle {
            role: TextRole::Caption,
            ..TextStyle::default()
        };
        ctx.draw_text(ctx.bounds(), style.run(caption), ctx.palette().success);
        let next = SECOND - spent % SECOND;
        ctx.wake_at(ctx.now() + Duration::from_millis(next.unsigned_abs()));
    }

    fn same(&self, other: &Self) -> bool {
        self.started == other.started
    }
}
