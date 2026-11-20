use std::sync::Mutex;

use proton_srp::{RawSRPModulus, ServerClientProof, ServerClientVerifier, ServerInteraction};
use rikka_proton_pgp::{Modulus, Srp};

pub const SIGNED_MODULUS: &str = "-----BEGIN PGP SIGNED MESSAGE-----
Hash: SHA256

y6TtufhYg2mIeauZYOti+GPbd/0vP66kP34TgE6elK/kXkTW/Yfrp1jMmtLiWWSq5cszTMRIEighuwPbZ/z3RrWPxsOg0+jYgbFu8yZ8vOAwrPtLxZl94x0PFTAZBrVapmCn+VYcM+UXdO9v70xFDLwj34tpPbvpODHVWHSlGlhOwndWg3XBE2D9PJopFZajNZiqOScBXree5rDgzU5BBaPbIb6nySpyaeThMCcNzpcEqE8r3ro+E/VdXBvSSJpusr1dvAwHc3IDGUzAhodqV5mjYy9nXwq/9gHWpYNtm76Ols7ReWAhZwy1+cQllQZwGfzzOVGpc+3WutOntQjM6Q==
-----BEGIN PGP SIGNATURE-----
Version: ProtonMail
Comment: https://protonmail.com

wl4EARYIABAFAlwB1j8JEDUFhcTpUY8mAADfEAD8DFdNXn4TsgbfbAZRDa9a
yywqa/2W9Qyg5MJaNZd2a+0BAPg04gEZI+G8RaoPVh/SYvWx7jpP3L1O8bEi
M/j1cjIO
=5RYw
-----END PGP SIGNATURE-----";

pub const MODULUS: &str = "y6TtufhYg2mIeauZYOti+GPbd/0vP66kP34TgE6elK/kXkTW/Yfrp1jMmtLiWWSq5cszTMRIEighuwPbZ/z3RrWPxsOg0+jYgbFu8yZ8vOAwrPtLxZl94x0PFTAZBrVapmCn+VYcM+UXdO9v70xFDLwj34tpPbvpODHVWHSlGlhOwndWg3XBE2D9PJopFZajNZiqOScBXree5rDgzU5BBaPbIb6nySpyaeThMCcNzpcEqE8r3ro+E/VdXBvSSJpusr1dvAwHc3IDGUzAhodqV5mjYy9nXwq/9gHWpYNtm76Ols7ReWAhZwy1+cQllQZwGfzzOVGpc+3WutOntQjM6Q==";

#[derive(Debug)]
pub struct FakeSrp {
    salt: String,
    modulus: RawSRPModulus,
    verifier: ServerClientVerifier,
    exchange: Mutex<Option<ServerInteraction>>,
}

impl FakeSrp {
    #[must_use]
    pub fn with_password(password: &str) -> FakeSrp {
        let registered = Srp::new()
            .register()
            .password(password)
            .modulus(Modulus::from_signed(SIGNED_MODULUS).unwrap())
            .verifier()
            .unwrap();

        FakeSrp::registered(registered.salt(), registered.verifier())
    }

    #[must_use]
    pub fn registered(salt: &str, verifier: &str) -> FakeSrp {
        let modulus = Modulus::from_signed(SIGNED_MODULUS).unwrap();

        FakeSrp {
            salt: salt.to_owned(),
            modulus: RawSRPModulus::new(modulus.as_str()).unwrap(),
            verifier: ServerClientVerifier::new(verifier).unwrap(),
            exchange: Mutex::new(None),
        }
    }

    #[must_use]
    pub fn salt(&self) -> &str {
        &self.salt
    }

    #[must_use]
    pub fn challenge(&self) -> String {
        let mut exchange = ServerInteraction::new(&self.modulus, &self.verifier).unwrap();
        let ephemeral = exchange.generate_challenge().encode_b64();

        *self.exchange.lock().unwrap() = Some(exchange);

        ephemeral
    }

    #[must_use]
    pub fn prove(&self, client_ephemeral: &str, client_proof: &str) -> Option<String> {
        let offered = ServerClientProof::new(client_ephemeral, client_proof).ok()?;

        self.exchange
            .lock()
            .unwrap()
            .as_mut()?
            .verify_proof(&offered)
            .ok()
            .map(|proof| proof.encode_b64())
    }
}
