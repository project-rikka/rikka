use crate::api::{LockSessionRequest, SessionCodeRequest, UserAccess};
use crate::client::{PREFIX, PassClient};
use crate::error::Result;

impl PassClient {
    pub async fn access(&self) -> Result<UserAccess> {
        let envelope: UserAccess = self
            .api()
            .get(format!("{PREFIX}/user/access"))
            .send_field("Access")
            .await?;

        Ok(envelope)
    }

    pub async fn lock_session(&self, code: impl Into<String>, unlocked_seconds: u32) -> Result<()> {
        let request = LockSessionRequest {
            lock_code: code.into(),
            unlocked_seconds,
        };

        self.api()
            .post(format!("{PREFIX}/user/session/lock"))
            .json(&request)
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn force_lock_session(&self) -> Result<()> {
        self.api()
            .post(format!("{PREFIX}/user/session/lock/force_lock"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn unlock_session(&self, code: impl Into<String>) -> Result<()> {
        let request = SessionCodeRequest {
            lock_code: code.into(),
        };

        self.api()
            .post(format!("{PREFIX}/user/session/lock/unlock"))
            .json(&request)
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn remove_session_lock(&self, code: impl Into<String>) -> Result<()> {
        let request = SessionCodeRequest {
            lock_code: code.into(),
        };

        self.api()
            .delete(format!("{PREFIX}/user/session/lock"))
            .json(&request)
            .send_unit()
            .await?;

        Ok(())
    }
}
