use rikka_compositor_shell::{ShellEvent, ToplevelEvent, ToplevelId};

use super::state::State;
use crate::event::Event;

impl State {
    pub(crate) fn shell_event_arrived(&mut self, event: ShellEvent) {
        match event {
            ShellEvent::Toplevel { toplevel, event } => self.toplevel_event(toplevel, event),
            ShellEvent::Popup { popup, event } => self.popup_event(popup, event),
            ShellEvent::Activate { surface, .. } => {
                if let Some(view) = self.view_of(surface) {
                    self.event_sender.send(Event::ActivationRequested { view });
                }
            }
            _ => {}
        }
    }

    fn toplevel_event(&mut self, toplevel: ToplevelId, event: ToplevelEvent) {
        let view = self
            .views
            .toplevels
            .get(toplevel)
            .and_then(|tracked| tracked.view);
        match event {
            ToplevelEvent::Created => self.toplevel_created(toplevel),
            ToplevelEvent::InitialCommit => {
                if view.is_none() {
                    self.unattached_follow(toplevel);
                }
                self.toplevel_initial_commit(toplevel);
                self.toplevel_announced(toplevel);
            }
            ToplevelEvent::AppId => self.toplevel_named(toplevel),
            ToplevelEvent::Title => self.toplevel_titled(toplevel),
            ToplevelEvent::Parent => self.toplevel_parented(toplevel),
            ToplevelEvent::Mapped | ToplevelEvent::Unmapped => match view {
                Some(view) => {
                    self.mark_dirty(view);
                    self.surfaces_follow(view);
                }
                None => self.dependent_changed(toplevel),
            },
            ToplevelEvent::Configured { serial } => self.toplevel_configured(toplevel, serial),
            ToplevelEvent::Destroyed => self.toplevel_destroyed(toplevel),
            ToplevelEvent::Fullscreen { .. } => {
                if let Some(view) = view {
                    self.event_sender.send(Event::FullscreenRequested {
                        view,
                        fullscreen: true,
                    });
                }
                self.toplevel_requested(toplevel);
            }
            ToplevelEvent::Unfullscreen => {
                if let Some(view) = view {
                    self.event_sender.send(Event::FullscreenRequested {
                        view,
                        fullscreen: false,
                    });
                }
                self.toplevel_requested(toplevel);
            }
            ToplevelEvent::Maximize | ToplevelEvent::Unmaximize => {
                self.toplevel_requested(toplevel)
            }
            _ => {}
        }
    }
}
