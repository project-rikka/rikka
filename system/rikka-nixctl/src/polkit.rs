use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_io::Timer;
use futures_util::future::{Either, select};
use rikka_bus::polkit::{AuthorityProxy, Subject};
use zbus::Connection;
use zbus::names::UniqueName;

use crate::error::{Error, Result};

pub(crate) const ACTION: &str = "os.rikka.nix.apply";

const ALLOW_USER_INTERACTION: u32 = 1;

const AUTHORIZATION_TIMEOUT: Duration = Duration::from_secs(300);

static NEXT_CANCELLATION_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) async fn is_authorized(
    connection: &Connection,
    caller: &UniqueName<'_>,
) -> Result<bool> {
    let subject = Subject::bus_name(caller.as_str()).map_err(Error::Subject)?;
    let authority = AuthorityProxy::new(connection)
        .await
        .map_err(Error::Polkit)?;

    let cancellation_id = format!(
        "rikka-nixctl-{}",
        NEXT_CANCELLATION_ID.fetch_add(1, Ordering::Relaxed)
    );
    let check = authority.check_authorization(
        &subject,
        ACTION,
        HashMap::new(),
        ALLOW_USER_INTERACTION,
        &cancellation_id,
    );
    match select(std::pin::pin!(check), Timer::after(AUTHORIZATION_TIMEOUT)).await {
        Either::Left((result, _)) => {
            let (authorized, _, _) = result.map_err(Error::Authorization)?;
            Ok(authorized)
        }
        Either::Right(_) => {
            if let Err(error) = authority.cancel_check_authorization(&cancellation_id).await {
                eprintln!("failed to cancel authorization check: {error}");
            }
            Err(Error::AuthorizationTimeout {
                minutes: AUTHORIZATION_TIMEOUT.as_secs() / 60,
            })
        }
    }
}
