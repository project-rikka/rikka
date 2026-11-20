use std::time::Duration;

use music_spotify_wire::{DeviceId, StoredCredentials};
use prost::Message as _;

use crate::error::{Error, Result};
use crate::identity::CLIENT_ID;
use crate::messages::login5::{
    ChallengeSolution, ChallengeSolutions, ClientInfo, HashcashSolution, LoginRequest,
    LoginResponse, ProtoDuration, StoredCredential,
};
use crate::token::{Progress, Token, lifetime};

const FLOW: &str = "login5";
const MAX_CHALLENGES: u32 = 3;
const FALLBACK_LIFETIME: Duration = Duration::from_secs(3600);
const TIMEOUT: i32 = 4;
const TOO_MANY_ATTEMPTS: i32 = 6;
const TRY_AGAIN_LATER: i32 = 8;

#[derive(Debug)]
pub struct Login5Exchange {
    request: LoginRequest,
    challenges: u32,
}

impl Login5Exchange {
    #[must_use]
    pub fn new(device: &DeviceId, credentials: &StoredCredentials) -> Login5Exchange {
        Login5Exchange {
            request: LoginRequest {
                client_info: Some(ClientInfo {
                    client_id: CLIENT_ID.to_owned(),
                    device_id: device.as_str().to_owned(),
                }),
                login_context: Vec::new(),
                challenge_solutions: None,
                stored_credential: Some(StoredCredential {
                    username: credentials.username.clone(),
                    data: credentials.auth_data.clone(),
                }),
            },
            challenges: 0,
        }
    }

    #[must_use]
    pub fn request(&self) -> Vec<u8> {
        self.request.encode_to_vec()
    }

    pub fn answer(&mut self, response: &[u8]) -> Result<Progress> {
        let response = LoginResponse::decode(response).map_err(Error::decode("login5 answer"))?;
        if let Some(ok) = response.ok {
            return Ok(Progress::Granted(Token::new(
                ok.access_token,
                lifetime(ok.access_token_expires_in, FALLBACK_LIFETIME),
            )));
        }
        if let Some(code) = response.error {
            return Err(match code {
                TIMEOUT | TOO_MANY_ATTEMPTS | TRY_AGAIN_LATER => Error::Throttled(FLOW),
                code => Error::TokenRefused {
                    flow: FLOW,
                    reason: reason(code).to_owned(),
                },
            });
        }
        let challenges = response
            .challenges
            .ok_or(Error::Malformed("login5 answer"))?;
        self.challenges += 1;
        if self.challenges > MAX_CHALLENGES {
            return Err(Error::Challenge(FLOW));
        }
        let mut solutions = Vec::new();
        for challenge in &challenges.challenges {
            if challenge.code.is_some() {
                return Err(Error::Challenge(FLOW));
            }
            let Some(hashcash) = &challenge.hashcash else {
                continue;
            };
            let zero_bits = u32::try_from(hashcash.length)
                .map_err(|_| Error::Malformed("login5 challenge length"))?;
            let solution =
                music_spotify_wire::solve(&response.login_context, &hashcash.prefix, zero_bits)
                    .ok_or(Error::Challenge(FLOW))?;
            solutions.push(ChallengeSolution {
                hashcash: Some(HashcashSolution {
                    suffix: solution.suffix.to_vec(),
                    duration: Some(ProtoDuration {
                        seconds: i64::try_from(solution.elapsed.as_secs()).unwrap_or(i64::MAX),
                        nanos: i32::try_from(solution.elapsed.subsec_nanos()).unwrap_or_default(),
                    }),
                }),
            });
        }
        self.request.challenge_solutions = Some(ChallengeSolutions { solutions });
        self.request.login_context = response.login_context;
        Ok(Progress::Challenged)
    }
}

fn reason(code: i32) -> &'static str {
    match code {
        1 => "invalid credentials",
        2 => "bad request",
        3 => "unsupported login protocol",
        5 => "unknown identifier",
        7 => "invalid phone number",
        _ => "unknown error",
    }
}
