use std::collections::HashMap;
use std::time::Instant;

use desk_layout::{Area, Widget, WidgetId, WidgetMatch, pair};
use rikka_types::Rect;

use crate::scene::transition_style::TransitionStyle;

#[derive(Debug, Default)]
pub(super) struct WidgetTransitions {
    last_layout: Option<LastLayout>,
    keys: HashMap<WidgetId, u64>,
    next: u64,
    transition_style: TransitionStyle,
    leaving: Vec<ExitingWidget>,
}

#[derive(Debug, PartialEq)]
struct LastLayout {
    wallpaper: Option<String>,
    widgets: Vec<Widget>,
    area: Area,
}

#[derive(Debug)]
pub(super) struct ExitingWidget {
    pub(super) key: u64,
    pub(super) widget: Widget,
    pub(super) rect: Rect,
    pub(super) transition_style: TransitionStyle,
    until: Option<Instant>,
}

pub(super) enum ExitProgress {
    Idle,
    Finished,
    NextDeadline(Instant),
}

impl WidgetTransitions {
    pub(super) fn follow(
        &mut self,
        wallpaper: Option<&str>,
        widgets: &[Widget],
        area: Area,
    ) -> TransitionStyle {
        let now = Instant::now();
        self.leaving
            .retain(|leaver| leaver.until.is_none_or(|until| until > now));
        let last_layout = LastLayout {
            wallpaper: wallpaper.map(str::to_owned),
            widgets: widgets.to_vec(),
            area,
        };
        let Some(before) = self.last_layout.take() else {
            for widget in widgets {
                let key = self.fresh();
                self.keys.insert(widget.id, key);
            }
            self.last_layout = Some(last_layout);
            return self.transition_style;
        };
        if before == last_layout {
            self.last_layout = Some(before);
            return self.transition_style;
        }
        let transition_style = match before.wallpaper == last_layout.wallpaper {
            true => TransitionStyle::Edit,
            false => TransitionStyle::Crossfade,
        };
        let pairing = match transition_style {
            TransitionStyle::Crossfade => pair(&before.widgets, widgets),
            TransitionStyle::Still | TransitionStyle::Edit => by_identity(&before.widgets, widgets),
        };
        self.recast(&before, widgets, pairing, transition_style);
        self.transition_style = transition_style;
        self.last_layout = Some(last_layout);
        transition_style
    }

    #[must_use]
    pub(super) fn key(&self, widget: WidgetId) -> u64 {
        self.keys[&widget]
    }

    #[must_use]
    pub(super) fn leaving(&self) -> &[ExitingWidget] {
        &self.leaving
    }

    pub(super) fn exit_progress(&mut self, now: Instant) -> ExitProgress {
        let mut next: Option<Instant> = None;
        for leaver in &mut self.leaving {
            let until = *leaver
                .until
                .get_or_insert_with(|| now + leaver.transition_style.leaving_for());
            if until <= now {
                return ExitProgress::Finished;
            }
            next = Some(next.map_or(until, |next| next.min(until)));
        }
        next.map_or(ExitProgress::Idle, ExitProgress::NextDeadline)
    }

    fn recast(
        &mut self,
        before: &LastLayout,
        widgets: &[Widget],
        pairing: WidgetMatch,
        transition_style: TransitionStyle,
    ) {
        let mut keys = HashMap::with_capacity(widgets.len());
        for (from, to) in pairing.matched {
            keys.insert(widgets[to].id, self.keys[&before.widgets[from].id]);
        }
        for came in pairing.added {
            let id = widgets[came].id;
            let key = match self.revive(id) {
                Some(key) => key,
                None => self.fresh(),
            };
            keys.insert(id, key);
        }
        for gone in pairing.removed {
            let widget = &before.widgets[gone];
            self.leaving.push(ExitingWidget {
                key: self.keys[&widget.id],
                widget: widget.clone(),
                rect: before.area.place(widget.size, widget.spot),
                transition_style,
                until: None,
            });
        }
        self.keys = keys;
    }

    fn revive(&mut self, widget: WidgetId) -> Option<u64> {
        let at = self
            .leaving
            .iter()
            .position(|leaver| leaver.widget.id == widget)?;
        Some(self.leaving.remove(at).key)
    }

    fn fresh(&mut self) -> u64 {
        self.next += 1;
        self.next
    }
}

fn by_identity(before: &[Widget], after: &[Widget]) -> WidgetMatch {
    let matched: Vec<(usize, usize)> = after
        .iter()
        .enumerate()
        .filter_map(|(to, widget)| {
            before
                .iter()
                .position(|earlier| earlier.id == widget.id)
                .map(|from| (from, to))
        })
        .collect();
    WidgetMatch {
        removed: (0..before.len())
            .filter(|from| !matched.iter().any(|(source, _)| source == from))
            .collect(),
        added: (0..after.len())
            .filter(|to| !matched.iter().any(|(_, target)| target == to))
            .collect(),
        matched,
    }
}
