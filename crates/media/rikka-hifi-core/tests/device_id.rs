use rikka_hifi_core::{DeviceId, NotDevice};

#[test]
fn pcm_names_parse_to_device_id() {
    for device in [
        DeviceId::new("Audio", 0),
        DeviceId::new("PCH", 3),
        DeviceId::new("odd,DEV=card", 12),
    ] {
        assert_eq!(device.pcm().parse::<DeviceId>(), Ok(device.clone()));
    }
}

#[test]
fn rejects_non_hw_names() {
    for named in [
        "",
        "shared",
        "hw:0,0",
        "hw:CARD=,DEV=0",
        "hw:CARD=Audio",
        "hw:CARD=Audio,DEV=",
        "hw:CARD=Audio,DEV=-1",
        "plughw:CARD=Audio,DEV=0",
    ] {
        assert_eq!(named.parse::<DeviceId>(), Err(NotDevice(named.to_owned())));
    }
}
