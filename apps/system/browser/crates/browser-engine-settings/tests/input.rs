use browser_engine_settings::{
    InputError, parse_bypass, parse_endpoint, parse_resolver, parse_script,
};
use rikka_web::Endpoint;

#[test]
fn endpoints_parse_host_and_port() {
    let cases = [
        ("", Ok(None)),
        (
            " proxy.test:3128 ",
            Ok(Some(Endpoint::new("proxy.test", 3128))),
        ),
        ("[::1]:1080", Ok(Some(Endpoint::new("::1", 1080)))),
        ("proxy.test", Err(InputError::InvalidPort)),
        ("proxy.test:0", Err(InputError::InvalidPort)),
        ("proxy.test:70000", Err(InputError::InvalidPort)),
        (":8080", Err(InputError::MissingHost)),
        ("user@proxy.test:8080", Err(InputError::MissingHost)),
    ];
    for (text, expected) in cases {
        assert_eq!(parse_endpoint(text), expected, "{text:?}");
    }
}

#[test]
fn addresses_are_checked() {
    assert_eq!(
        parse_resolver("https://dns.test/dns-query"),
        Ok("https://dns.test/dns-query".to_owned())
    );
    assert_eq!(
        parse_resolver("http://dns.test/dns-query"),
        Err(InputError::NotHttps)
    );
    assert_eq!(parse_resolver("dns.test"), Err(InputError::MalformedUrl));
    assert_eq!(
        parse_script("file:///etc/proxy.pac"),
        Ok("file:///etc/proxy.pac".to_owned())
    );
    assert_eq!(
        parse_script("ftp://pac.test/proxy.pac"),
        Err(InputError::UnsupportedScheme)
    );
}

#[test]
fn bypass_list_splits_on_commas_and_spaces() {
    assert_eq!(
        parse_bypass("localhost, 127.0.0.1\n.internal.test,,"),
        ["localhost", "127.0.0.1", ".internal.test"]
    );
}
