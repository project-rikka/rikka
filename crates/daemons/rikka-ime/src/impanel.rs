use std::sync::Arc;

use rikka_types::Rect;

use crate::inbox::Inbox;

pub(crate) struct Impanel {
    inbox: Arc<Inbox>,
}

impl Impanel {
    pub(crate) fn new(inbox: Arc<Inbox>) -> Impanel {
        Impanel { inbox }
    }
}

#[zbus::interface(name = "org.kde.impanel2")]
impl Impanel {
    fn set_spot_rect(&self, x: i32, y: i32, width: i32, height: i32) {
        let spot = Rect::new(x as f32, y as f32, width as f32, height as f32);
        self.inbox.update(|state| state.set_spot(spot));
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "org.kde.impanel2 SetLookupTable has seven arguments"
    )]
    fn set_lookup_table(
        &self,
        labels: Vec<String>,
        texts: Vec<String>,
        _attributes: Vec<String>,
        previous: bool,
        more: bool,
        cursor: i32,
        layout: i32,
    ) {
        self.inbox
            .update(|state| state.set_page(labels, texts, previous, more, cursor, layout));
    }
}

impl std::fmt::Debug for Impanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Impanel").finish_non_exhaustive()
    }
}
