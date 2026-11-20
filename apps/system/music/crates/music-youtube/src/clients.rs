use serde_json::{Map, Value, json};

const DESKTOP_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";
const VISION_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";
const MUSIC_ORIGIN: &str = "https://music.youtube.com";
const YOUTUBE_ORIGIN: &str = "https://www.youtube.com";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Client {
    Music,
    VisionOs,
}

impl Client {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Client::Music => "WEB_REMIX",
            Client::VisionOs => "VISIONOS",
        }
    }

    pub(crate) fn version(self) -> &'static str {
        match self {
            Client::Music => "1.20260707.12.00",
            Client::VisionOs => "1.02",
        }
    }

    pub(crate) fn number(self) -> u32 {
        match self {
            Client::Music => 67,
            Client::VisionOs => 101,
        }
    }

    pub(crate) fn agent(self) -> &'static str {
        match self {
            Client::Music => DESKTOP_AGENT,
            Client::VisionOs => VISION_AGENT,
        }
    }

    pub(crate) fn origin(self) -> &'static str {
        match self {
            Client::Music => MUSIC_ORIGIN,
            Client::VisionOs => YOUTUBE_ORIGIN,
        }
    }

    pub(crate) fn context(self, visitor: Option<&str>, language: &str, region: &str) -> Value {
        let mut client = Map::new();
        client.insert("hl".into(), json!(language));
        client.insert("gl".into(), json!(region));
        client.insert("clientName".into(), json!(self.name()));
        client.insert("clientVersion".into(), json!(self.version()));
        client.insert("userAgent".into(), json!(self.agent()));
        client.insert("utcOffsetMinutes".into(), json!(0));
        if let Some(visitor) = visitor {
            client.insert("visitorData".into(), json!(visitor));
        }
        let device = match self {
            Client::Music => json!({
                "osName": "Windows",
                "osVersion": "10.0",
                "platform": "DESKTOP",
                "clientFormFactor": "UNKNOWN_FORM_FACTOR",
                "browserName": "Chrome",
                "browserVersion": "125.0.0.0",
                "originalUrl": self.origin(),
            }),
            Client::VisionOs => json!({
                "deviceMake": "Apple",
                "deviceModel": "RealityDevice17,1",
                "osName": "visionOS",
                "osVersion": "26.5.23O471",
            }),
        };
        if let Value::Object(device) = device {
            client.extend(device);
        }
        json!({
            "client": client,
            "user": { "lockedSafetyMode": false },
            "request": { "useSsl": true, "internalExperimentFlags": [] },
        })
    }
}
