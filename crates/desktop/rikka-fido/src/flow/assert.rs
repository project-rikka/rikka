use super::devices::connect_all;
use super::select::choose;
use super::verify::Verifier;
use crate::authenticator::Authenticator;
use crate::cbor::Value;
use crate::ceremony::{Prompter, Watch};
use crate::crypto;
use crate::ctap2::{
    self, Ask, AuthData, GET_ASSERTION_PERMISSION, HMAC_SECRET, Info, LARGE_BLOB_KEY, PinProtocol,
    SecretAsk,
};
use crate::error::{Error, Result};
use crate::hid::Transport;
use crate::status::CtapStatus;
use crate::u2f;
use crate::webauthn::{Assertion, PreparedAssertion, SecretUse, UserVerification};

struct Target {
    rp_id: String,
    allow: Vec<Vec<u8>>,
    appid: bool,
}

pub(crate) fn run(
    transport: &dyn Transport,
    request: &PreparedAssertion,
    watch: &Watch,
    prompter: &Prompter,
) -> Result<Assertion> {
    let authenticators = connect_all(transport, watch)?;
    let alone = authenticators.len() == 1;

    let mut holders = Vec::new();
    let mut targets = Vec::new();
    for mut authenticator in authenticators {
        match locate(&mut authenticator, request, alone, watch) {
            Ok(Some(target)) => {
                holders.push(authenticator);
                targets.push(target);
            }
            Ok(None) => {}
            Err(error) if error.is_stop() => return Err(error),
            Err(error) => {
                tracing::warn!(device = authenticator.device().path(), %error, "failed to query security key credentials");
            }
        }
    }

    if holders.is_empty() {
        return Err(Error::NoCredentials);
    }

    let (index, mut authenticator) = choose(holders, watch)?;
    let target = targets.swap_remove(index);

    match authenticator.ctap2() {
        Some(info) => sign(&mut authenticator, &info, &target, request, watch, prompter),
        None => sign_u2f(&mut authenticator, &target, request, watch),
    }
}

fn locate(
    authenticator: &mut Authenticator,
    request: &PreparedAssertion,
    alone: bool,
    watch: &Watch,
) -> Result<Option<Target>> {
    let mut relying_parties = vec![(request.rp_id.clone(), false)];
    if let Some(appid) = &request.appid {
        relying_parties.push((appid.clone(), true));
    }

    let Some(info) = authenticator.ctap2() else {
        return locate_u2f(authenticator, request, &relying_parties, watch);
    };

    if request.allow.is_empty() {
        return Ok(Some(Target {
            rp_id: request.rp_id.clone(),
            allow: Vec::new(),
            appid: false,
        }));
    }

    let allow: Vec<Vec<u8>> = request
        .allow
        .iter()
        .filter(|id| info.accepts_credential_id(id))
        .cloned()
        .collect();
    if allow.is_empty() {
        return Ok(None);
    }

    let single_needed = !request.extensions.prf_by_credential.is_empty() && allow.len() > 1;
    if alone && request.appid.is_none() && !single_needed && allow.len() <= info.batch() {
        return Ok(Some(Target {
            rp_id: request.rp_id.clone(),
            allow,
            appid: false,
        }));
    }

    for (rp_id, appid) in &relying_parties {
        for batch in allow.chunks(info.batch()) {
            match probe(authenticator, rp_id, batch, request, watch) {
                Ok(Some(found)) => {
                    return Ok(Some(Target {
                        rp_id: rp_id.clone(),
                        allow: vec![found],
                        appid: *appid,
                    }));
                }
                Ok(None) => {}
                Err(Error::Ctap(_)) => {
                    return Ok(Some(Target {
                        rp_id: request.rp_id.clone(),
                        allow: allow[..allow.len().min(info.batch())].to_vec(),
                        appid: false,
                    }));
                }
                Err(error) => return Err(error),
            }
        }
    }

    Ok(requires_uv_to_list(&info, request).then(|| Target {
        rp_id: request.rp_id.clone(),
        allow: allow[..allow.len().min(info.batch())].to_vec(),
        appid: false,
    }))
}

fn probe(
    authenticator: &mut Authenticator,
    rp_id: &str,
    batch: &[Vec<u8>],
    request: &PreparedAssertion,
    watch: &Watch,
) -> Result<Option<Vec<u8>>> {
    let ask = Ask {
        rp_id,
        client_data_hash: &request.client_data_hash,
        allow: batch,
        presence: false,
        built_in_uv: false,
        auth: None,
        extensions: None,
    };

    match ctap2::get_assertion(authenticator.channel(), &ask, watch) {
        Ok(signed) => match (signed.credential, batch) {
            (Some(found), _) => Ok(Some(found)),
            (None, [only]) => Ok(Some(only.clone())),
            (None, _) => Err(Error::Protocol("assertion missing credential")),
        },
        Err(Error::Ctap(CtapStatus::NO_CREDENTIALS)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn requires_uv_to_list(info: &Info, request: &PreparedAssertion) -> bool {
    let verifiable = info.has_pin() == Some(true) || info.has_built_in_uv() == Some(true);
    verifiable && request.user_verification == UserVerification::Required
}

fn locate_u2f(
    authenticator: &mut Authenticator,
    request: &PreparedAssertion,
    relying_parties: &[(String, bool)],
    watch: &Watch,
) -> Result<Option<Target>> {
    if request.user_verification == UserVerification::Required {
        return Ok(None);
    }

    for (rp_id, appid) in relying_parties {
        let application = crypto::sha256(rp_id);
        for id in request
            .allow
            .iter()
            .filter(|id| u2f::is_valid_key_handle(id))
        {
            if u2f::has_key_handle(authenticator.channel(), &application, id, watch)? {
                return Ok(Some(Target {
                    rp_id: rp_id.clone(),
                    allow: vec![id.clone()],
                    appid: *appid,
                }));
            }
        }
    }

    Ok(None)
}

fn sign(
    authenticator: &mut Authenticator,
    info: &Info,
    target: &Target,
    request: &PreparedAssertion,
    watch: &Watch,
    prompter: &Prompter,
) -> Result<Assertion> {
    let mut verifier = Verifier::new(
        info.clone(),
        request.user_verification,
        &target.rp_id,
        GET_ASSERTION_PERMISSION,
        prompter,
        watch,
    )?;

    let salts = if info.supports_extension(HMAC_SECRET) {
        let only = match target.allow.as_slice() {
            [only] => Some(only.as_slice()),
            _ => None,
        };
        request.extensions.salts_for(only)
    } else {
        None
    };
    let read_blob = request.extensions.large_blob_read && ctap2::supports_large_blobs(info);
    let protocol = info.pin_protocol().unwrap_or(PinProtocol::One);

    let (first, secret) = loop {
        let proof = verifier.proof(authenticator.channel())?;
        let secret = salts
            .map(|(salts, _)| SecretAsk::prepare(authenticator.channel(), protocol, &salts, watch))
            .transpose()?;
        let ask = Ask {
            rp_id: &target.rp_id,
            client_data_hash: &request.client_data_hash,
            allow: &target.allow,
            presence: true,
            built_in_uv: proof.built_in(),
            auth: proof.sign(&request.client_data_hash),
            extensions: extensions(secret.as_ref(), read_blob),
        };
        match ctap2::get_assertion(authenticator.channel(), &ask, watch) {
            Ok(signed) => break (signed, secret),
            Err(Error::Ctap(CtapStatus::NO_CREDENTIALS)) => return Err(Error::NoCredentials),
            Err(error) => verifier.recover(error)?,
        }
    };

    let signed = if target.allow.is_empty() && first.count > 1 {
        let mut all = vec![first];
        for _ in 1..all[0].count {
            all.push(ctap2::get_next_assertion(authenticator.channel(), watch)?);
        }
        let users = all
            .iter()
            .map(|signed| {
                signed
                    .user
                    .clone()
                    .ok_or(Error::Protocol("discoverable assertion missing user"))
            })
            .collect::<Result<Vec<_>>>()?;
        let index = prompter.account(watch, users)?;
        all.swap_remove(index)
    } else {
        first
    };

    let credential_id = match (&signed.credential, target.allow.as_slice()) {
        (Some(id), _) => id.clone(),
        (None, [only]) => only.clone(),
        (None, _) => return Err(Error::Protocol("assertion missing credential")),
    };

    let parsed = AuthData::parse(&signed.auth_data)?;
    if parsed.rp_id_hash != crypto::sha256(&target.rp_id) {
        return Err(Error::Protocol("assertion rpIdHash does not match"));
    }

    let outputs = match (&secret, parsed.extensions.get(HMAC_SECRET)) {
        (Some(secret), Some(sealed)) => Some(secret.open(sealed)?),
        _ => None,
    };
    let (prf, hmac_secret) = match salts.map(|(_, using)| using) {
        Some(SecretUse::Prf) => (outputs, None),
        Some(SecretUse::Raw) => (None, outputs),
        None => (None, None),
    };

    let large_blob = match (&signed.large_blob_key, read_blob) {
        (Some(key), true) => ctap2::read_large_blob(authenticator.channel(), info, key, watch)?,
        _ => None,
    };

    Ok(Assertion {
        credential_id,
        client_data_json: request.client_data_json.clone(),
        authenticator_data: signed.auth_data,
        signature: signed.signature,
        user: signed.user,
        appid: target.appid,
        prf,
        hmac_secret,
        large_blob,
    })
}

fn extensions(secret: Option<&SecretAsk>, read_blob: bool) -> Option<Value> {
    let mut entries = Vec::new();
    if let Some(secret) = secret {
        entries.push((Value::from(HMAC_SECRET), secret.input()));
    }
    if read_blob {
        entries.push((Value::from(LARGE_BLOB_KEY), Value::Bool(true)));
    }
    (!entries.is_empty()).then_some(Value::Map(entries))
}

fn sign_u2f(
    authenticator: &mut Authenticator,
    target: &Target,
    request: &PreparedAssertion,
    watch: &Watch,
) -> Result<Assertion> {
    let [handle] = target.allow.as_slice() else {
        return Err(Error::NoCredentials);
    };
    let application = crypto::sha256(&target.rp_id);
    let signed = u2f::authenticate(
        authenticator.channel(),
        &request.client_data_hash,
        &application,
        handle,
        watch,
    )?;

    let mut authenticator_data = Vec::with_capacity(37);
    authenticator_data.extend_from_slice(&application);
    authenticator_data.push(signed.flags);
    authenticator_data.extend_from_slice(&signed.counter);

    Ok(Assertion {
        credential_id: handle.clone(),
        client_data_json: request.client_data_json.clone(),
        authenticator_data,
        signature: signed.signature,
        user: None,
        appid: target.appid,
        prf: None,
        hmac_secret: None,
        large_blob: None,
    })
}
