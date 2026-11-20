use crate::ceremony::{Event, NewPinReason, Pin, Prompter, Watch};
use crate::ctap2::{self, Info, PinProtocol, Scope, Token};
use crate::ctaphid::Channel;
use crate::error::{Error, Result};
use crate::status::CtapStatus;
use crate::webauthn::UserVerification;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Method {
    Skip,
    BuiltIn,
    UvToken,
    Pin,
}

pub(crate) enum Proof {
    Nothing,
    BuiltIn,
    Token(Token),
}

impl Proof {
    pub(crate) fn built_in(&self) -> bool {
        matches!(self, Proof::BuiltIn)
    }

    pub(crate) fn sign(&self, client_data_hash: &[u8; 32]) -> Option<(Vec<u8>, PinProtocol)> {
        match self {
            Proof::Token(token) => Some((token.sign(client_data_hash), token.protocol())),
            Proof::Nothing | Proof::BuiltIn => None,
        }
    }
}

pub(crate) struct Verifier<'a> {
    info: Info,
    rp_id: &'a str,
    permissions: u8,
    prompter: &'a Prompter,
    watch: &'a Watch,
    method: Method,
    refreshed: bool,
    uv_failed: bool,
    wrong_pin: bool,
    rejected_pin: bool,
    known_pin: Option<Pin>,
}

impl<'a> Verifier<'a> {
    pub(crate) fn new(
        info: Info,
        requirement: UserVerification,
        rp_id: &'a str,
        permissions: u8,
        prompter: &'a Prompter,
        watch: &'a Watch,
    ) -> Result<Verifier<'a>> {
        let needed = match requirement {
            UserVerification::Required => true,
            UserVerification::Preferred => strongest(&info).is_some(),
            UserVerification::Discouraged => info.option("alwaysUv") == Some(true),
        };

        let method = match (needed, strongest(&info)) {
            (false, _) => Method::Skip,
            (true, Some(method)) => method,
            (true, None) if requirement == UserVerification::Required => can_set_pin(&info)
                .then_some(Method::Pin)
                .ok_or_else(|| unavailable(&info))?,
            (true, None) => Method::Skip,
        };

        Ok(Verifier {
            info,
            rp_id,
            permissions,
            prompter,
            watch,
            method,
            refreshed: false,
            uv_failed: false,
            wrong_pin: false,
            rejected_pin: false,
            known_pin: None,
        })
    }

    pub(crate) fn proof(&mut self, channel: &mut Channel) -> Result<Proof> {
        loop {
            match self.method {
                Method::Skip => return Ok(Proof::Nothing),
                Method::BuiltIn => {
                    let retries = self.uv_retries(channel)?;
                    self.watch.emit(Event::Verify { retries });
                    return Ok(Proof::BuiltIn);
                }
                Method::UvToken => {
                    if let Some(token) = self.uv_token(channel)? {
                        return Ok(Proof::Token(token));
                    }
                }
                Method::Pin => return self.pin_token(channel).map(Proof::Token),
            }
        }
    }

    pub(crate) fn recover(&mut self, error: Error) -> Result<()> {
        let Error::Ctap(status) = error else {
            return Err(error);
        };

        match (status, self.method) {
            (CtapStatus::PUAT_REQUIRED, Method::Skip) => {
                self.method = strongest(&self.info)
                    .or_else(|| can_set_pin(&self.info).then_some(Method::Pin))
                    .ok_or_else(|| unavailable(&self.info))?;
                Ok(())
            }
            (CtapStatus::PIN_AUTH_INVALID, Method::Pin | Method::UvToken) if !self.refreshed => {
                self.refreshed = true;
                Ok(())
            }
            (CtapStatus::UV_INVALID, Method::BuiltIn) => {
                self.uv_failed = true;
                Ok(())
            }
            (CtapStatus::UV_BLOCKED, Method::BuiltIn | Method::UvToken) => self.fall_back_to_pin(),
            (CtapStatus::PIN_BLOCKED, _) => Err(Error::PinBlocked),
            (CtapStatus::PIN_AUTH_BLOCKED, _) => Err(Error::PinAuthBlocked),
            (CtapStatus::PIN_NOT_SET, _) if can_set_pin(&self.info) => {
                self.method = Method::Pin;
                Ok(())
            }
            (CtapStatus::PIN_NOT_SET, _) => Err(Error::PinNotSet),
            _ => Err(Error::Ctap(status)),
        }
    }

    fn fall_back_to_pin(&mut self) -> Result<()> {
        if self.info.has_pin() == Some(true) {
            self.method = Method::Pin;
            Ok(())
        } else {
            Err(Error::UvBlocked)
        }
    }

    fn protocol(&self) -> Result<PinProtocol> {
        self.info
            .pin_protocol()
            .ok_or(Error::Protocol("no supported PIN/UV auth protocol"))
    }

    fn scope(&self) -> Scope<'a> {
        Scope {
            permissions: self.permissions,
            rp_id: self.rp_id,
        }
    }

    fn uv_retries(&self, channel: &mut Channel) -> Result<Option<u32>> {
        if !self.uv_failed {
            return Ok(None);
        }
        let protocol = self.protocol()?;
        ctap2::uv_retries(channel, protocol, self.watch).map(Some)
    }

    fn uv_token(&mut self, channel: &mut Channel) -> Result<Option<Token>> {
        let retries = self.uv_retries(channel)?;
        self.watch.emit(Event::Verify { retries });

        match ctap2::uv_token(channel, self.protocol()?, self.scope(), self.watch) {
            Ok(token) => Ok(Some(token)),
            Err(Error::Ctap(CtapStatus::UV_INVALID)) => {
                self.uv_failed = true;
                Ok(None)
            }
            Err(Error::Ctap(CtapStatus::UV_BLOCKED)) => {
                self.fall_back_to_pin()?;
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    fn pin_token(&mut self, channel: &mut Channel) -> Result<Token> {
        let protocol = self.protocol()?;

        loop {
            let min_length = self.info.min_pin_length();

            if self.info.has_pin() == Some(false) {
                let new = self.new_pin(NewPinReason::NotSet, min_length)?;
                match ctap2::set_pin(channel, protocol, &new, self.watch) {
                    Ok(()) => self.known_pin = Some(new),
                    Err(Error::Ctap(CtapStatus::PIN_POLICY_VIOLATION)) => self.rejected_pin = true,
                    Err(Error::Ctap(CtapStatus::NOT_ALLOWED)) => {}
                    Err(error) => return Err(ctap2::pin_failure(error, min_length)),
                }
                self.refresh(channel)?;
                continue;
            }

            let retries = ctap2::pin_retries(channel, protocol, self.watch)?;
            if retries == 0 {
                return Err(Error::PinBlocked);
            }

            let pin = match self.known_pin.take() {
                Some(pin) => pin,
                None => self
                    .prompter
                    .pin(self.watch, retries, min_length, self.wrong_pin)?,
            };
            if !pin.is_valid_length(0) {
                self.wrong_pin = true;
                continue;
            }

            if self.info.force_pin_change() {
                let new = self.new_pin(NewPinReason::ChangeRequired, min_length)?;
                match ctap2::change_pin(channel, protocol, &pin, &new, self.watch) {
                    Ok(()) => {
                        self.wrong_pin = false;
                        self.known_pin = Some(new);
                        self.refresh(channel)?;
                    }
                    Err(Error::Ctap(CtapStatus::PIN_INVALID)) => self.wrong_pin = true,
                    Err(Error::Ctap(CtapStatus::PIN_POLICY_VIOLATION)) => {
                        self.rejected_pin = true;
                        self.known_pin = Some(pin);
                    }
                    Err(error) => return Err(ctap2::pin_failure(error, min_length)),
                }
                continue;
            }

            let scope = self.info.supports_pin_uv_auth_token().then(|| self.scope());
            match ctap2::pin_token(channel, protocol, &pin, scope, self.watch) {
                Ok(token) => {
                    self.wrong_pin = false;
                    return Ok(token);
                }
                Err(Error::Ctap(CtapStatus::PIN_INVALID)) => self.wrong_pin = true,
                Err(Error::Ctap(CtapStatus::PIN_POLICY_VIOLATION)) => {
                    self.refresh(channel)?;
                    if !self.info.force_pin_change() {
                        return Err(Error::PinPolicyViolation { min_length });
                    }
                    self.known_pin = Some(pin);
                }
                Err(error) => return Err(ctap2::pin_failure(error, min_length)),
            }
        }
    }

    fn new_pin(&mut self, reason: NewPinReason, min_length: usize) -> Result<Pin> {
        loop {
            let pin = self
                .prompter
                .new_pin(self.watch, min_length, reason, self.rejected_pin)?;
            self.rejected_pin = !pin.is_valid_length(min_length);
            if !self.rejected_pin {
                return Ok(pin);
            }
        }
    }

    fn refresh(&mut self, channel: &mut Channel) -> Result<()> {
        self.info = Info::read(channel, self.watch)?;
        Ok(())
    }
}

fn strongest(info: &Info) -> Option<Method> {
    if info.has_built_in_uv() == Some(true) {
        return Some(if info.supports_pin_uv_auth_token() {
            Method::UvToken
        } else {
            Method::BuiltIn
        });
    }

    (info.has_pin() == Some(true) && !pin_lacks_mc_ga_permission(info)).then_some(Method::Pin)
}

fn can_set_pin(info: &Info) -> bool {
    info.has_pin() == Some(false) && !pin_lacks_mc_ga_permission(info)
}

fn pin_lacks_mc_ga_permission(info: &Info) -> bool {
    info.supports_pin_uv_auth_token() && info.option("noMcGaPermissionsWithClientPin") == Some(true)
}

fn unavailable(info: &Info) -> Error {
    if info.has_pin() == Some(false) {
        Error::PinNotSet
    } else {
        Error::UserVerificationUnavailable
    }
}
