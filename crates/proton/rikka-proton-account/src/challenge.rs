use rikka_proton_api::{Api, AppVersion, offload};
use serde_json::{Map, Value, json};

use crate::device_profile::DeviceProfile;
use crate::field_activity::FieldActivity;

const DEFAULT_PLATFORM: &str = "linux";
const FRAME_SUFFIX: &str = "v4-challenge";

#[derive(Debug, Clone, PartialEq)]
pub struct Challenge {
    prefix: String,
    device: DeviceProfile,
}

impl Challenge {
    #[must_use]
    pub fn new(app_version: &AppVersion, device: DeviceProfile) -> Challenge {
        Challenge {
            prefix: frame_prefix(app_version),
            device,
        }
    }

    pub async fn collect(api: &Api) -> Challenge {
        let app_version = api.app_version().clone();

        offload(move || Challenge::new(&app_version, DeviceProfile::collect())).await
    }

    #[must_use]
    pub fn frame_prefix(&self) -> &str {
        &self.prefix
    }

    #[must_use]
    pub fn device(&self) -> &DeviceProfile {
        &self.device
    }

    #[must_use]
    pub fn device_payload(&self) -> Value {
        self.payload([frame(&self.device, None)])
    }

    #[must_use]
    pub fn login_payload(&self, username: Option<&FieldActivity>) -> Value {
        let field = username.map(|activity| FieldFrame::username(activity));

        self.payload([frame(&self.device, field)])
    }

    #[must_use]
    pub fn signup_payload(&self, username: &FieldActivity, recovery: &FieldActivity) -> Value {
        self.payload([
            frame(&self.device, Some(FieldFrame::username(username))),
            frame(
                &self.device,
                Some(FieldFrame {
                    name: "recovery",
                    suffix: "Recovery",
                    activity: recovery,
                }),
            ),
        ])
    }

    fn payload<const N: usize>(&self, frames: [Value; N]) -> Value {
        let frames: Map<String, Value> = frames
            .into_iter()
            .enumerate()
            .map(|(index, frame)| (format!("{}-{index}", self.prefix), frame))
            .collect();

        Value::Object(frames)
    }
}

struct FieldFrame<'a> {
    name: &'static str,
    suffix: &'static str,
    activity: &'a FieldActivity,
}

impl FieldFrame<'_> {
    fn username(activity: &FieldActivity) -> FieldFrame<'_> {
        FieldFrame {
            name: "username",
            suffix: "Username",
            activity,
        }
    }
}

fn frame(device: &DeviceProfile, field: Option<FieldFrame<'_>>) -> Value {
    let mut frame = json!(device);

    if let (Some(field), Value::Object(fields)) = (field, &mut frame) {
        let FieldFrame {
            name,
            suffix,
            activity,
        } = field;
        fields.insert("frame".to_owned(), json!({ "name": name }));
        fields.insert(format!("time{suffix}"), json!(activity.focus_seconds));
        fields.insert(format!("click{suffix}"), json!(activity.clicks));
        fields.insert(format!("copy{suffix}"), json!(activity.copies));
        fields.insert(format!("paste{suffix}"), json!(activity.pastes));
        fields.insert(format!("keydown{suffix}"), json!(activity.keys));
    }

    frame
}

fn frame_prefix(app_version: &AppVersion) -> String {
    let client = app_version.client();
    let (platform, product) = client.split_once('-').unwrap_or((DEFAULT_PLATFORM, client));

    format!("{product}-{platform}-{FRAME_SUFFIX}")
}
