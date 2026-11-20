use rikka_proton_pgp::Secret;
use serde::Serialize;

use crate::account::Account;
use crate::error::Result;
use crate::session_info::SessionInfo;

const MDM_LOGIN_PATH: &str = "vpn/v1/business/mdm-login";

#[must_use]
#[derive(Debug)]
pub struct MdmLogin {
    account: Account,
    token: Secret,
    group: String,
    device: Option<String>,
}

impl MdmLogin {
    pub fn device(mut self, unique_id: impl Into<String>) -> Self {
        self.device = Some(unique_id.into());
        self
    }

    pub async fn execute(self) -> Result<SessionInfo> {
        self.account.ensure_session().await?;

        let request = MdmLoginRequest {
            token: self.token.expose(),
            group: &self.group,
            device: self.device.as_deref(),
        };
        let session: SessionInfo = self
            .account
            .api()
            .post(MDM_LOGIN_PATH)
            .json(&request)
            .send()
            .await?;

        self.account.api().set_session(session.session())?;

        Ok(session)
    }

    pub(crate) fn new(account: Account, token: Secret, group: String) -> MdmLogin {
        MdmLogin {
            account,
            token,
            group,
            device: None,
        }
    }
}

#[derive(Serialize)]
struct MdmLoginRequest<'a> {
    #[serde(rename = "MdmToken")]
    token: &'a str,
    #[serde(rename = "Group")]
    group: &'a str,
    #[serde(rename = "DeviceUniqId", skip_serializing_if = "Option::is_none")]
    device: Option<&'a str>,
}
