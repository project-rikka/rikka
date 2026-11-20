use rikka_remote_link::Pin;
use rikka_secrets::{Entry, Secrets, schema};

const HOST: &str = "host";

fn entry(pin: Pin) -> Entry {
    Entry::new(schema::REMOTE).attribute(HOST, pin.to_hex())
}

pub(crate) async fn token(pin: Pin) -> Option<Vec<u8>> {
    let found = async { Secrets::shared().await?.lookup(&entry(pin)).await };
    match found.await {
        Ok(found) => found.map(|secret| secret.expose().to_vec()),
        Err(error) => {
            tracing::debug!("no remembered sign-in: {error}");
            None
        }
    }
}

pub(crate) async fn keep(pin: Pin, label: String, token: Vec<u8>) {
    let kept = async {
        Secrets::shared()
            .await?
            .store(&entry(pin), &label, &token)
            .await
    };
    if let Err(error) = kept.await {
        tracing::warn!("cannot remember the sign-in: {error}");
    }
}

pub(crate) async fn forget(pin: Pin) {
    let removed = async { Secrets::shared().await?.remove(&entry(pin)).await };
    if let Err(error) = removed.await {
        tracing::warn!("cannot forget the sign-in: {error}");
    }
}
