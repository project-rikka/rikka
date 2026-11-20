use rikka_sync::Mutex;

use futures_channel::mpsc;
use futures_util::{Stream, StreamExt, stream};
use rikka_bus::{Rule, Signals};
use zbus::blocking::Connection;

use crate::controller::{INTERFACE, PATH};
use crate::error::{Error, Result};
use crate::names;

pub(crate) fn start() -> Result<impl Stream<Item = ()> + Send + 'static> {
    let connection = Connection::session().map_err(Error::bus)?;
    let (notify, changes) = mpsc::channel::<()>(1);
    let notify = Mutex::new(notify);
    let watch = Signals::new()
        .rule(names::owner_changed())
        .rule(
            Rule::new()
                .sender(names::FCITX)
                .path(PATH)
                .interface(INTERFACE),
        )
        .each(
            &connection,
            |_| Some(()),
            move |()| {
                let mut notify = notify.lock();
                !matches!(notify.try_send(()), Err(full) if full.is_disconnected())
            },
        )
        .map_err(Error::bus)?;
    Ok(stream::unfold(
        (changes, watch, connection),
        |(mut changes, watch, connection)| async move {
            changes
                .next()
                .await
                .map(|()| ((), (changes, watch, connection)))
        },
    ))
}
