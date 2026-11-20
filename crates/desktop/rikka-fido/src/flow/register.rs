use super::devices::connect_all;
use super::select::choose;
use super::verify::Verifier;
use crate::authenticator::Authenticator;
use crate::cbor::Value;
use crate::ceremony::{Prompter, Watch};
use crate::crypto;
use crate::ctap2::{
    self, Ask, Auth, AuthData, HMAC_SECRET, HMAC_SECRET_MC, Info, LARGE_BLOB_KEY,
    MAKE_CREDENTIAL_PERMISSION, Make, PinProtocol, SecretAsk,
};
use crate::error::{Error, Result};
use crate::hid::Transport;
use crate::status::CtapStatus;
use crate::u2f;
use crate::webauthn::{
    Algorithm, Attestation, CreateInputs, CreateOutputs, CredentialProtection, LargeBlobSupport,
    PreparedRegistration, PrfOutput, Registration, ResidentKey, Salts, UserVerification,
};

const U2F_FORMAT: &str = "fido-u2f";
const PRESENT_AND_ATTESTED: u8 = 0x41;
const CRED_PROTECT: &str = "credProtect";
const MIN_PIN_LENGTH: &str = "minPinLength";

struct Plan {
    cred_protect: Option<CredentialProtection>,
    hmac_secret: bool,
    secret_salts: Option<Salts>,
    min_pin_length: bool,
    large_blob_key: bool,
}

pub(crate) fn run(
    transport: &dyn Transport,
    request: &PreparedRegistration,
    watch: &Watch,
    prompter: &Prompter,
) -> Result<Registration> {
    let authenticators = connect_all(transport, watch)?;
    let (_, mut authenticator) = choose(authenticators, watch)?;

    match authenticator.ctap2() {
        Some(info) => make(&mut authenticator, &info, request, watch, prompter),
        None => make_u2f(&mut authenticator, request, watch),
    }
}

fn make(
    authenticator: &mut Authenticator,
    info: &Info,
    request: &PreparedRegistration,
    watch: &Watch,
    prompter: &Prompter,
) -> Result<Registration> {
    let algorithms: Vec<Algorithm> = if info.algorithms().is_empty() {
        request.algorithms.clone()
    } else {
        request
            .algorithms
            .iter()
            .copied()
            .filter(|algorithm| info.algorithms().contains(&algorithm.id()))
            .collect()
    };
    if algorithms.is_empty() {
        return Err(Error::UnsupportedAlgorithm);
    }

    let stores = info.option("rk") == Some(true);
    let resident = match request.resident_key {
        ResidentKey::Required if !stores => return Err(Error::ResidentKeyUnavailable),
        ResidentKey::Required => true,
        ResidentKey::Preferred => stores,
        ResidentKey::Discouraged => false,
    };
    let verification = if resident && request.user_verification == UserVerification::Discouraged {
        UserVerification::Preferred
    } else {
        request.user_verification
    };

    let plan = plan(info, &request.extensions, resident)?;
    let exclude = exclusions(authenticator, info, request, watch)?;
    let protocol = info.pin_protocol().unwrap_or(PinProtocol::One);

    let mut verifier = Verifier::new(
        info.clone(),
        verification,
        request.rp.id(),
        MAKE_CREDENTIAL_PERMISSION,
        prompter,
        watch,
    )?;

    let (made, secret) = loop {
        let proof = verifier.proof(authenticator.channel())?;
        let secret = plan
            .secret_salts
            .map(|salts| SecretAsk::prepare(authenticator.channel(), protocol, &salts, watch))
            .transpose()?;
        let make = Make {
            client_data_hash: &request.client_data_hash,
            rp: &request.rp,
            user: &request.user,
            algorithms: &algorithms,
            exclude: &exclude,
            resident,
            built_in_uv: proof.built_in(),
            auth: proof
                .sign(&request.client_data_hash)
                .map(|(param, protocol)| Auth::Signed(param, protocol)),
            extensions: extensions(&plan, secret.as_ref()),
        };
        match ctap2::make_credential(authenticator.channel(), &make, watch) {
            Ok(made) => break (made, secret),
            Err(Error::Ctap(CtapStatus::CREDENTIAL_EXCLUDED)) => {
                return Err(Error::CredentialExcluded);
            }
            Err(Error::Ctap(CtapStatus::UNSUPPORTED_ALGORITHM)) => {
                return Err(Error::UnsupportedAlgorithm);
            }
            Err(error) => verifier.recover(error)?,
        }
    };

    let parsed = AuthData::parse(&made.auth_data)?;
    if parsed.rp_id_hash != crypto::sha256(request.rp.id()) {
        return Err(Error::Protocol("credential rpIdHash does not match"));
    }
    let attested = parsed
        .credential
        .ok_or(Error::Protocol("attestation missing credential data"))?;

    let inputs = &request.extensions;
    let returned = &parsed.extensions;
    let secret_made = returned.get(HMAC_SECRET).and_then(Value::as_bool) == Some(true);
    let prf_results = match (&secret, returned.get(HMAC_SECRET_MC)) {
        (Some(secret), Some(sealed)) => Some(secret.open(sealed)?),
        _ => None,
    };
    let outputs = CreateOutputs {
        cred_props: inputs.cred_props.then_some(resident),
        cred_protect: returned
            .get(CRED_PROTECT)
            .and_then(Value::as_unsigned)
            .and_then(CredentialProtection::from_level),
        hmac_create_secret: inputs.hmac_create_secret.then_some(secret_made),
        prf: inputs.prf.as_ref().map(|_| PrfOutput {
            enabled: secret_made,
            results: prf_results,
        }),
        min_pin_length: returned
            .get(MIN_PIN_LENGTH)
            .and_then(Value::as_unsigned)
            .and_then(|length| u32::try_from(length).ok()),
        large_blob_supported: inputs.large_blob.map(|_| made.large_blob_key.is_some()),
    };
    if inputs.large_blob == Some(LargeBlobSupport::Required) && made.large_blob_key.is_none() {
        return Err(Error::ExtensionUnsupported("largeBlob"));
    }

    let attestation = Attestation {
        format: made.format,
        statement: made.statement,
        auth_data: made.auth_data,
    }
    .with_conveyance(request.attestation);

    Ok(Registration {
        credential_id: attested.id,
        client_data_json: request.client_data_json.clone(),
        attestation_object: attestation.object(),
        authenticator_data: attestation.auth_data,
        public_key: attested.public_key,
        algorithm: Algorithm::new(attested.algorithm),
        format: attestation.format,
        discoverable: resident,
        extensions: outputs,
    })
}

fn plan(info: &Info, inputs: &CreateInputs, resident: bool) -> Result<Plan> {
    let cred_protect = match inputs.cred_protect {
        Some((policy, _)) if info.supports_extension(CRED_PROTECT) => Some(policy),
        Some((policy, true)) if policy != CredentialProtection::UserVerificationOptional => {
            return Err(Error::ExtensionUnsupported(CRED_PROTECT));
        }
        _ => None,
    };

    let hmac_secret = inputs.wants_hmac_secret() && info.supports_extension(HMAC_SECRET);
    let secret_salts = match &inputs.prf {
        Some(Some(eval)) if hmac_secret && info.supports_extension(HMAC_SECRET_MC) => {
            Some(eval.salts())
        }
        _ => None,
    };

    let large_blob_key = match inputs.large_blob {
        Some(support) => {
            let able = resident && ctap2::supports_large_blobs(info);
            if !able && support == LargeBlobSupport::Required {
                return Err(Error::ExtensionUnsupported("largeBlob"));
            }
            able
        }
        None => false,
    };

    Ok(Plan {
        cred_protect,
        hmac_secret,
        secret_salts,
        min_pin_length: inputs.min_pin_length && info.supports_extension(MIN_PIN_LENGTH),
        large_blob_key,
    })
}

fn extensions(plan: &Plan, secret: Option<&SecretAsk>) -> Option<Value> {
    let mut entries = Vec::new();
    if let Some(policy) = plan.cred_protect {
        entries.push((Value::from(CRED_PROTECT), Value::from(policy.level())));
    }
    if plan.hmac_secret {
        entries.push((Value::from(HMAC_SECRET), Value::Bool(true)));
    }
    if let Some(secret) = secret {
        entries.push((Value::from(HMAC_SECRET_MC), secret.input()));
    }
    if plan.min_pin_length {
        entries.push((Value::from(MIN_PIN_LENGTH), Value::Bool(true)));
    }
    if plan.large_blob_key {
        entries.push((Value::from(LARGE_BLOB_KEY), Value::Bool(true)));
    }
    (!entries.is_empty()).then_some(Value::Map(entries))
}

fn exclusions(
    authenticator: &mut Authenticator,
    info: &Info,
    request: &PreparedRegistration,
    watch: &Watch,
) -> Result<Vec<Vec<u8>>> {
    let exclude: Vec<Vec<u8>> = request
        .exclude
        .iter()
        .filter(|id| info.accepts_credential_id(id))
        .cloned()
        .collect();
    if exclude.len() <= info.batch() {
        return Ok(exclude);
    }

    for batch in exclude.chunks(info.batch()) {
        let ask = Ask {
            rp_id: request.rp.id(),
            client_data_hash: &request.client_data_hash,
            allow: batch,
            presence: false,
            built_in_uv: false,
            auth: None,
            extensions: None,
        };
        match ctap2::get_assertion(authenticator.channel(), &ask, watch) {
            Ok(_) => return Err(Error::CredentialExcluded),
            Err(Error::Ctap(CtapStatus::NO_CREDENTIALS)) => {}
            Err(error) => return Err(error),
        }
    }

    Ok(Vec::new())
}

fn make_u2f(
    authenticator: &mut Authenticator,
    request: &PreparedRegistration,
    watch: &Watch,
) -> Result<Registration> {
    if request.resident_key == ResidentKey::Required {
        return Err(Error::ResidentKeyUnavailable);
    }
    if request.user_verification == UserVerification::Required {
        return Err(Error::UserVerificationUnavailable);
    }
    if !request.algorithms.contains(&Algorithm::ES256) {
        return Err(Error::UnsupportedAlgorithm);
    }
    plan(&Info::default(), &request.extensions, false)?;
    let inputs = &request.extensions;
    let outputs = CreateOutputs {
        cred_props: inputs.cred_props.then_some(false),
        hmac_create_secret: inputs.hmac_create_secret.then_some(false),
        prf: inputs.prf.as_ref().map(|_| PrfOutput {
            enabled: false,
            results: None,
        }),
        large_blob_supported: inputs.large_blob.map(|_| false),
        ..CreateOutputs::default()
    };

    let application = crypto::sha256(request.rp.id());
    for id in request
        .exclude
        .iter()
        .filter(|id| u2f::is_valid_key_handle(id))
    {
        if u2f::has_key_handle(authenticator.channel(), &application, id, watch)? {
            return Err(Error::CredentialExcluded);
        }
    }

    let registered = u2f::register(
        authenticator.channel(),
        &request.client_data_hash,
        &application,
        watch,
    )?;

    let public_key = registered.cose_key().encode();
    let mut authenticator_data =
        Vec::with_capacity(55 + registered.handle.len() + public_key.len());
    authenticator_data.extend_from_slice(&application);
    authenticator_data.push(PRESENT_AND_ATTESTED);
    authenticator_data.extend_from_slice(&[0; 4]);
    authenticator_data.extend_from_slice(&[0; 16]);
    authenticator_data.extend_from_slice(&(registered.handle.len() as u16).to_be_bytes());
    authenticator_data.extend_from_slice(&registered.handle);
    authenticator_data.extend_from_slice(&public_key);

    let statement = Value::map([
        ("sig", Value::bytes(registered.signature.clone())),
        (
            "x5c",
            Value::array([Value::bytes(registered.certificate.clone())]),
        ),
    ]);
    let attestation = Attestation {
        format: U2F_FORMAT.to_owned(),
        statement,
        auth_data: authenticator_data,
    }
    .with_conveyance(request.attestation);

    Ok(Registration {
        credential_id: registered.handle,
        client_data_json: request.client_data_json.clone(),
        attestation_object: attestation.object(),
        authenticator_data: attestation.auth_data,
        public_key,
        algorithm: Algorithm::ES256,
        format: attestation.format,
        discoverable: false,
        extensions: outputs,
    })
}
