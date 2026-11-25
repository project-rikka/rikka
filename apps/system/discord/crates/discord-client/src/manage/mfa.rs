use discord_core::{MfaChallenge, MfaMethod};
use discord_wire as wire;
use serde_json::json;

use crate::client::Client;

impl Client {
    pub async fn send_mfa_sms(&self, challenge: &MfaChallenge) -> discord_core::Result<()> {
        let ticket = challenge.ticket.clone();
        self.work(move |session| async move {
            session
                .rest()
                .post("/auth/mfa/sms/send")
                .json(json!({ "ticket": ticket }))
                .done()
                .await
        })
        .await
    }

    pub async fn finish_mfa(
        &self,
        challenge: &MfaChallenge,
        method: MfaMethod,
        code: &str,
    ) -> discord_core::Result<()> {
        let ticket = challenge.ticket.clone();
        let data = match method {
            MfaMethod::Password => code.to_owned(),
            MfaMethod::Totp | MfaMethod::Sms | MfaMethod::Backup => {
                code.trim().replace([' ', '-'], "")
            }
        };
        self.work(move |session| async move {
            let token: wire::MfaToken = session
                .rest()
                .post("/mfa/finish")
                .json(json!({ "ticket": ticket, "mfa_type": method.name(), "data": data }))
                .parse()
                .await?;
            session.rest().elevate(token.token);
            Ok(())
        })
        .await
    }
}
