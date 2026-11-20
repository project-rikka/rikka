use std::time::Duration;

use music_spotify_wire::DeviceId;
use prost::Message as _;

use crate::error::{Error, Result};
use crate::identity::{CLIENT_ID, CLIENT_VERSION};
use crate::messages::client_token::{
    ChallengeAnswer, ChallengeAnswersRequest, ClientDataRequest, ClientTokenRequest,
    ClientTokenResponse, ConnectivitySdkData, HashCashAnswer, NativeDesktopLinuxData,
    PlatformSpecificData,
};
use crate::platform::Platform;
use crate::token::{Progress, Token, lifetime};

const FLOW: &str = "client token";
const CLIENT_DATA_REQUEST: i32 = 1;
const CHALLENGE_ANSWERS_REQUEST: i32 = 2;
const GRANTED_TOKEN_RESPONSE: i32 = 1;
const CHALLENGES_RESPONSE: i32 = 2;
const HASH_CASH: i32 = 3;
const MAX_CHALLENGES: u32 = 3;
const FALLBACK_LIFETIME: Duration = Duration::from_secs(7200);

#[derive(Debug)]
pub struct ClientTokenExchange {
    request: ClientTokenRequest,
    challenges: u32,
}

impl ClientTokenExchange {
    #[must_use]
    pub fn new(device: &DeviceId, platform: &Platform) -> ClientTokenExchange {
        ClientTokenExchange {
            request: ClientTokenRequest {
                request_type: CLIENT_DATA_REQUEST,
                client_data: Some(ClientDataRequest {
                    client_version: CLIENT_VERSION.to_owned(),
                    client_id: CLIENT_ID.to_owned(),
                    connectivity_sdk_data: Some(ConnectivitySdkData {
                        platform_specific_data: Some(PlatformSpecificData {
                            desktop_linux: Some(NativeDesktopLinuxData {
                                system_name: platform.system_name.clone(),
                                system_release: platform.system_release.clone(),
                                system_version: platform.system_version.clone(),
                                hardware: platform.hardware.clone(),
                            }),
                        }),
                        device_id: device.as_str().to_owned(),
                    }),
                }),
                challenge_answers: None,
            },
            challenges: 0,
        }
    }

    #[must_use]
    pub fn request(&self) -> Vec<u8> {
        self.request.encode_to_vec()
    }

    pub fn answer(&mut self, response: &[u8]) -> Result<Progress> {
        let response =
            ClientTokenResponse::decode(response).map_err(Error::decode("client token answer"))?;
        match response.response_type {
            GRANTED_TOKEN_RESPONSE => {
                let granted = response
                    .granted_token
                    .ok_or(Error::Malformed("granted client token"))?;
                Ok(Progress::Granted(Token::new(
                    granted.token,
                    lifetime(granted.refresh_after_seconds, FALLBACK_LIFETIME),
                )))
            }
            CHALLENGES_RESPONSE => {
                self.challenges += 1;
                if self.challenges > MAX_CHALLENGES {
                    return Err(Error::Challenge(FLOW));
                }
                let challenges = response
                    .challenges
                    .ok_or(Error::Malformed("client token challenges"))?;
                let parameters = challenges
                    .challenges
                    .iter()
                    .find(|challenge| challenge.challenge_type == HASH_CASH)
                    .and_then(|challenge| challenge.evaluate_hashcash_parameters.as_ref())
                    .ok_or(Error::Challenge(FLOW))?;
                let prefix = rikka_types::hex::decode(&parameters.prefix)
                    .map_err(|_| Error::Malformed("client token challenge prefix"))?;
                let zero_bits = u32::try_from(parameters.length)
                    .map_err(|_| Error::Malformed("client token challenge length"))?;
                let solution = music_spotify_wire::solve(&[], &prefix, zero_bits)
                    .ok_or(Error::Challenge(FLOW))?;
                self.request = ClientTokenRequest {
                    request_type: CHALLENGE_ANSWERS_REQUEST,
                    client_data: None,
                    challenge_answers: Some(ChallengeAnswersRequest {
                        state: challenges.state,
                        answers: vec![ChallengeAnswer {
                            challenge_type: HASH_CASH,
                            hash_cash: Some(HashCashAnswer {
                                suffix: rikka_types::hex::encode(solution.suffix)
                                    .to_ascii_uppercase(),
                            }),
                        }],
                    }),
                };
                Ok(Progress::Challenged)
            }
            other => Err(Error::TokenRefused {
                flow: FLOW,
                reason: format!("answer type {other}"),
            }),
        }
    }
}
