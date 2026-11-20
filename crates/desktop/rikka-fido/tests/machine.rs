use rikka_fido::{Error, Fido};

#[test]
fn plugged_keys_answer_init_and_get_info() {
    let fido = Fido::new();
    let devices = fido.devices().expect("hidraw enumerates");

    for device in devices {
        let mut authenticator = match fido.open(&device) {
            Ok(authenticator) => authenticator,
            Err(Error::AccessDenied(path)) => {
                eprintln!("{path} is not accessible to this user; skipping it");
                continue;
            }
            Err(error) => panic!("{} does not answer: {error}", device.path()),
        };

        match authenticator.info() {
            Some(info) => {
                assert!(!info.versions().is_empty(), "{}", device.path());
                if info.has_pin() == Some(true) {
                    let retries = authenticator.pin_retries().expect("PIN retries");
                    assert!(retries.is_some(), "{}", device.path());
                }
            }
            None => assert!(authenticator.is_u2f_only()),
        }

        authenticator.wink().expect("the key winks or declines");
    }
}
