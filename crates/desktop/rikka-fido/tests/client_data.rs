use rikka_fido::{ClientData, ClientDataType};

use crate::support::sha256;

#[test]
fn client_data_serializes_like_webauthn() {
    let cases = [
        (
            ClientData::new(
                ClientDataType::Get,
                [0x01, 0x02, 0xff],
                "https://example.com",
            ),
            r#"{"type":"webauthn.get","challenge":"AQL_","origin":"https://example.com","crossOrigin":false}"#,
        ),
        (
            ClientData::new(ClientDataType::Create, [0xfb, 0xff], "https://a\"b\\\u{1}")
                .cross_origin("https://top.example"),
            r#"{"type":"webauthn.create","challenge":"-_8","origin":"https://a\"b\\\u0001","crossOrigin":true,"topOrigin":"https://top.example"}"#,
        ),
    ];

    for (client_data, expected) in cases {
        assert_eq!(client_data.json(), expected);
        assert_eq!(client_data.hash(), sha256(expected));
    }
}
