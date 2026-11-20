use std::sync::mpsc;
use std::thread;

use crate::authenticator::Authenticator;
use crate::ceremony::{Event, Watch};
use crate::ctap2::{self, Auth, FIDO_2_1, Make, SELECTION};
use crate::error::{Error, Result};
use crate::status::CtapStatus;
use crate::u2f;
use crate::webauthn::{Algorithm, RelyingParty, User};

const DUMMY: [u8; 32] = [0x41; 32];
const DUMMY_RP: &str = ".dummy";

pub(crate) fn choose(
    mut candidates: Vec<Authenticator>,
    watch: &Watch,
) -> Result<(usize, Authenticator)> {
    if candidates.len() == 1 {
        let only = candidates.pop().expect("one candidate");
        return Ok((0, only));
    }
    if candidates.is_empty() {
        return Err(Error::NoCredentials);
    }

    watch.emit(Event::Choose);
    let (branch, settled) = watch.fork();
    let (sender, receiver) = mpsc::channel();

    let mut workers = Vec::with_capacity(candidates.len());
    for (index, mut candidate) in candidates.into_iter().enumerate() {
        let branch = branch.clone();
        let sender = sender.clone();
        let worker = thread::Builder::new()
            .name("rikka-fido-select".to_owned())
            .spawn(move || {
                let picked = wait_for_selection(&mut candidate, &branch);
                if sender.send((index, picked, candidate)).is_err() {
                    tracing::debug!("security key selection already settled");
                }
            });
        match worker {
            Ok(worker) => workers.push(worker),
            Err(error) => {
                settled.cancel();
                for worker in workers {
                    join_selector(worker);
                }
                return Err(Error::Spawn(error));
            }
        }
    }
    drop(sender);

    let mut winner = None;
    let mut failure = None;
    for (index, picked, candidate) in receiver {
        match picked {
            Ok(()) if winner.is_none() => {
                settled.cancel();
                winner = Some((index, candidate));
            }
            Ok(()) => {}
            Err(error) if error.is_stop() => {}
            Err(error) => failure = Some(error),
        }
    }
    for worker in workers {
        join_selector(worker);
    }

    if let Some(winner) = winner {
        return Ok(winner);
    }
    watch.check()?;
    Err(failure.unwrap_or(Error::Cancelled))
}

fn wait_for_selection(authenticator: &mut Authenticator, watch: &Watch) -> Result<()> {
    let Some(info) = authenticator.ctap2() else {
        return u2f::register(authenticator.channel(), &DUMMY, &DUMMY, watch).map(drop);
    };

    if info.supports(FIDO_2_1) {
        return ctap2::call(authenticator.channel(), SELECTION, None, watch).map(drop);
    }

    let rp = RelyingParty::new(DUMMY_RP);
    let user = User::new(vec![0x01]).with_name("dummy");
    let make = Make {
        client_data_hash: &DUMMY,
        rp: &rp,
        user: &user,
        algorithms: &[Algorithm::ES256],
        exclude: &[],
        resident: false,
        built_in_uv: false,
        auth: info.has_pin().map(|_| Auth::Blank),
        extensions: None,
    };

    match ctap2::make_credential(authenticator.channel(), &make, watch) {
        Ok(_) => Ok(()),
        Err(Error::Ctap(CtapStatus::PIN_INVALID | CtapStatus::PIN_NOT_SET)) => Ok(()),
        Err(error) => Err(error),
    }
}

fn join_selector(worker: thread::JoinHandle<()>) {
    if worker.join().is_err() {
        tracing::error!("security key selection thread panicked");
    }
}
