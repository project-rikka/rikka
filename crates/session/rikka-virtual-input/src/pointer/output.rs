use rikka_wayland::{Display, EventLoop};
use rikka_wayland_client::wl_output::{self, WlOutput};

use crate::error::{Error, Result};

const NAMED_SINCE: u32 = 4;
const RELEASE_SINCE: u32 = 3;

#[derive(Default)]
pub(super) struct Names(Vec<(u32, String)>);

pub(super) struct NamedOutput {
    events: EventLoop<Names>,
    names: Names,
    output: WlOutput,
}

impl NamedOutput {
    pub(super) fn find(display: &Display, name: &str) -> Result<NamedOutput> {
        let mut events = display.event_loop::<Names>();
        events.on::<WlOutput>(on_wl_output);
        let outputs: Vec<WlOutput> = display.bind_all(&events.route(), NAMED_SINCE);
        let mut names = Names::default();
        events.settle(&mut names)?;
        let mut found = None;
        for output in outputs {
            if found.is_none() && names.of(&output) == Some(name) {
                found = Some(output);
            } else if output.version() >= RELEASE_SINCE {
                output.release();
            }
        }
        let output = found.ok_or_else(|| Error::OutputNotFound(name.to_owned()))?;
        Ok(NamedOutput {
            events,
            names,
            output,
        })
    }

    pub(super) fn output(&self) -> &WlOutput {
        &self.output
    }

    pub(super) fn dispatch(&mut self) -> Result<()> {
        self.events.dispatch_pending(&mut self.names)?;
        Ok(())
    }

    pub(super) fn release(&self) {
        if self.output.version() >= RELEASE_SINCE {
            self.output.release();
        }
    }
}

impl Names {
    fn of(&self, output: &WlOutput) -> Option<&str> {
        self.0
            .iter()
            .find(|(id, _)| *id == output.id())
            .map(|(_, name)| name.as_str())
    }
}

fn on_wl_output(names: &mut Names, output: WlOutput, event: wl_output::Event) {
    if let wl_output::Event::Name { name } = event {
        names.0.push((output.id(), name));
    }
}
