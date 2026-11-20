use std::time::Duration;

use futures_util::StreamExt;
use rikka_network::{Failure, Joining, Network, Progress};
use rikka_seam::SeamClient;
use rikka_ui::tr;
use shell_kit::detached;
use shell_notifications::Announcement;

const PATIENCE: Duration = Duration::from_secs(45);

struct Strings {
    app_name: String,
    joined: String,
    not_joined: String,
    secrets: String,
    handshake: String,
    address: String,
    dropped: String,
    missing: String,
    silent: String,
}

impl Strings {
    fn now(network: &str) -> Strings {
        Strings {
            app_name: tr!("shell.capture.scan.name").to_string(),
            joined: tr!("shell.capture.scan.joined", network = network).to_string(),
            not_joined: tr!("shell.capture.scan.not-joined", network = network).to_string(),
            secrets: tr!("shell.capture.scan.failure.secrets").to_string(),
            handshake: tr!("shell.capture.scan.failure.handshake").to_string(),
            address: tr!("shell.capture.scan.failure.address").to_string(),
            dropped: tr!("shell.capture.scan.failure.dropped").to_string(),
            missing: tr!("shell.capture.scan.failure.missing").to_string(),
            silent: tr!("shell.capture.scan.failure.silent").to_string(),
        }
    }

    fn failure(&self, failure: Failure) -> &str {
        match failure {
            Failure::Secrets => &self.secrets,
            Failure::Handshake => &self.handshake,
            Failure::Address => &self.address,
            Failure::Missing => &self.missing,
            Failure::None | Failure::Carrier | Failure::Other => &self.dropped,
        }
    }
}

enum Outcome {
    Joined,
    Failed(String),
}

pub(crate) fn follow(network: Network, joining: Joining, seam: SeamClient) {
    let words = Strings::now(&joining.ssid().name());
    detached("rikka-capture-join", move || {
        let outcome = wait(&network, joining, &words);
        let announcement = match outcome {
            Outcome::Joined => Announcement::new(&words.app_name, &words.joined),
            Outcome::Failed(reason) => Announcement::new(&words.app_name, &words.not_joined)
                .body(reason)
                .urgent(),
        };
        announcement.send(&seam);
    });
}

fn wait(network: &Network, mut joining: Joining, words: &Strings) -> Outcome {
    let changes = network.changes();
    let first = network.state();
    let settled = rikka_worker::block_on_timeout(
        async move {
            let mut states =
                std::pin::pin!(futures_util::stream::once(async { first }).chain(changes));
            while let Some(state) = states.next().await {
                match joining.follow(&state) {
                    Progress::Pending => continue,
                    Progress::Joined => return Outcome::Joined,
                    Progress::Failed(failure) => {
                        return Outcome::Failed(words.failure(failure).to_owned());
                    }
                    Progress::Refused(refusal) => return Outcome::Failed(refusal),
                }
            }
            Outcome::Failed(words.missing.clone())
        },
        PATIENCE,
    );
    settled.unwrap_or_else(|| Outcome::Failed(words.silent.clone()))
}
