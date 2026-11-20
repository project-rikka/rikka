use rikka_remote_wire::DEFAULT_PORT;
use rikka_settings::session::remote::PORT_DEFAULT;

#[test]
fn session_setting_defaults_to_the_protocol_port() {
    assert_eq!(i64::from(DEFAULT_PORT), PORT_DEFAULT);
}
