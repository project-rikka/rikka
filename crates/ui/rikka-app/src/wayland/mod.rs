pub(crate) mod activation;
pub(crate) mod client;
pub(crate) mod configure;
pub(crate) mod data_device;
mod dispatch;
mod drag;
pub(crate) mod feedback;
pub(crate) mod globals;
mod hosting;
pub(crate) mod inhibit;
pub(crate) mod locked;
mod looping;
mod open;
pub(crate) mod outputs;
mod runner;
pub(crate) mod scale;
mod session;
pub(crate) mod settings;
pub(crate) mod token;
pub(crate) mod toplevel;
mod window_state;

use crate::error::Result;
use crate::launch::Launch;
use crate::tasks::LoopEvent;
use crate::wayland::client::Client;
use crate::wayland::runner::{Runner, RunnerConfig};

pub(crate) fn run(launch: Launch) -> Result<()> {
    let Launch {
        app_name,
        device,
        videos,
        theme,
        appearance,
        language,
        windows,
        policy,
        arriving,
        proxy,
        mailbox,
        wake,
    } = launch;
    let following = proxy.clone();
    settings::follow(
        move |change| {
            LoopEvent::from_session(change).is_none_or(|event| following.send_event(event).is_ok())
        },
        appearance,
        language,
    );
    let (client, queue) = Client::connect()?;
    let mut runner = Runner::new(
        RunnerConfig {
            app_name,
            device,
            videos,
            theme,
            windows,
            policy,
            arriving,
            proxy,
        },
        client,
    );
    let outcome = looping::run(&mut runner, queue, &mailbox, wake);
    let stored = runner.finish();
    outcome?;
    stored
}
