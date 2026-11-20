use std::fmt;
use std::time::Duration;

use crate::ceremony::{Pin, Watch};
use crate::ctap2::{self, Info};
use crate::ctaphid::Channel;
use crate::error::{Error, Result};
use crate::hid::{Device, HidLink};
use crate::status::CtapStatus;

const QUERY_TIME: Duration = Duration::from_secs(5);

pub struct Authenticator {
    device: Device,
    channel: Channel,
    info: Option<Info>,
}

impl Authenticator {
    pub(crate) fn connect(
        device: Device,
        link: Box<dyn HidLink>,
        watch: &Watch,
    ) -> Result<Authenticator> {
        let mut channel = Channel::open(link, watch)?;

        let info = if channel.supports_cbor() {
            match Info::read(&mut channel, watch) {
                Ok(info) => Some(info),
                Err(Error::Ctap(CtapStatus::INVALID_COMMAND) | Error::Hid(0x01))
                    if channel.supports_msg() =>
                {
                    None
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };

        if info.is_none() && !channel.supports_msg() {
            return Err(Error::Protocol("device supports neither CTAP2 nor U2F"));
        }

        Ok(Authenticator {
            device,
            channel,
            info,
        })
    }

    #[must_use]
    pub fn device(&self) -> &Device {
        &self.device
    }

    #[must_use]
    pub fn info(&self) -> Option<&Info> {
        self.info.as_ref()
    }

    #[must_use]
    pub fn is_u2f_only(&self) -> bool {
        self.info.is_none()
    }

    pub fn wink(&mut self) -> Result<bool> {
        if !self.channel.supports_wink() {
            return Ok(false);
        }
        self.channel.wink(&Watch::bounded(QUERY_TIME))?;
        Ok(true)
    }

    pub fn pin_retries(&mut self) -> Result<Option<u32>> {
        let Some(protocol) = self.info.as_ref().and_then(Info::pin_protocol) else {
            return Ok(None);
        };
        ctap2::pin_retries(&mut self.channel, protocol, &Watch::bounded(QUERY_TIME)).map(Some)
    }

    pub fn uv_retries(&mut self) -> Result<Option<u32>> {
        let Some(info) = &self.info else {
            return Ok(None);
        };
        if info.has_built_in_uv().is_none() {
            return Ok(None);
        }
        let protocol = info.pin_protocol().unwrap_or(ctap2::PinProtocol::Two);
        ctap2::uv_retries(&mut self.channel, protocol, &Watch::bounded(QUERY_TIME)).map(Some)
    }

    pub fn set_pin(&mut self, pin: impl Into<Pin>) -> Result<()> {
        let (protocol, min_length) = self.pin_policy(Some(false))?;
        let pin = pin.into();
        if !pin.is_valid_length(min_length) {
            return Err(Error::PinPolicyViolation { min_length });
        }

        let watch = Watch::bounded(QUERY_TIME);
        ctap2::set_pin(&mut self.channel, protocol, &pin, &watch)
            .map_err(|error| ctap2::pin_failure(error, min_length))?;
        self.refresh(&watch)
    }

    pub fn change_pin(&mut self, current: impl Into<Pin>, new: impl Into<Pin>) -> Result<()> {
        let (protocol, min_length) = self.pin_policy(Some(true))?;
        let new = new.into();
        if !new.is_valid_length(min_length) {
            return Err(Error::PinPolicyViolation { min_length });
        }

        let watch = Watch::bounded(QUERY_TIME);
        match ctap2::change_pin(&mut self.channel, protocol, &current.into(), &new, &watch) {
            Ok(()) => self.refresh(&watch),
            Err(Error::Ctap(CtapStatus::PIN_INVALID)) => {
                let retries = ctap2::pin_retries(&mut self.channel, protocol, &watch)?;
                Err(Error::WrongPin { retries })
            }
            Err(error) => Err(ctap2::pin_failure(error, min_length)),
        }
    }

    fn pin_policy(&self, expected: Option<bool>) -> Result<(ctap2::PinProtocol, usize)> {
        let info = self.info.as_ref().ok_or(Error::PinUnsupported)?;
        let protocol = info.pin_protocol().ok_or(Error::PinUnsupported)?;
        match info.has_pin() {
            None => Err(Error::PinUnsupported),
            found if found == expected => Ok((protocol, info.min_pin_length())),
            Some(true) => Err(Error::PinAlreadySet),
            Some(false) => Err(Error::PinNotSet),
        }
    }

    fn refresh(&mut self, watch: &Watch) -> Result<()> {
        self.info = Some(Info::read(&mut self.channel, watch)?);
        Ok(())
    }

    pub(crate) fn channel(&mut self) -> &mut Channel {
        &mut self.channel
    }

    pub(crate) fn ctap2(&self) -> Option<Info> {
        self.info.clone()
    }
}

impl fmt::Debug for Authenticator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Authenticator")
            .field("device", &self.device)
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}
