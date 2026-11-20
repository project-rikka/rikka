use std::sync::{Arc, Mutex};
use std::time::Duration;

use rikka_proton_api::{Api, Environment, Error, Multipart, Part};

use rikka_test_proton::{Request, response, serve};

fn api(address: &str) -> Api {
    Api::builder()
        .app_version("linux-mail@1.0.0")
        .environment(Environment::custom(address).unwrap())
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn multipart_form_follows_rfc_7578() {
    let last_request: Arc<Mutex<Option<Request>>> = Arc::new(Mutex::new(None));
    let server_request = last_request.clone();
    let address = serve(move |request: Request| {
        *server_request.lock().unwrap() = Some(request);
        response(200, r#"{"Code":1000}"#)
    })
    .await;

    api(&address)
        .post("mail/v4/attachments")
        .header("content-type", "application/json")
        .multipart(
            Multipart::new()
                .text("Filename", "a \"quoted\"\r\nname.txt")
                .file("DataPacket", "blob", vec![0u8, 1, 2, 255])
                .part(
                    "Note",
                    Part::text("plain words").content_type("text/plain; charset=utf-8"),
                ),
        )
        .send_unit()
        .await
        .unwrap();

    let request = last_request.lock().unwrap().take().unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/mail/v4/attachments");

    let content_type = request.header("content-type").unwrap();
    let boundary = content_type
        .strip_prefix("multipart/form-data; boundary=")
        .unwrap();
    assert!(
        (1..=70).contains(&boundary.len())
            && boundary
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
        "{boundary} is not a plain RFC 2046 boundary"
    );

    let mut expected = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"Filename\"\r\n\
         \r\n\
         a \"quoted\"\r\nname.txt\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"DataPacket\"; filename=\"blob\"\r\n\
         Content-Type: application/octet-stream\r\n\
         \r\n"
    )
    .into_bytes();
    expected.extend_from_slice(&[0, 1, 2, 255]);
    expected.extend_from_slice(
        format!(
            "\r\n--{boundary}\r\n\
             Content-Disposition: form-data; name=\"Note\"\r\n\
             Content-Type: text/plain; charset=utf-8\r\n\
             \r\n\
             plain words\r\n\
             --{boundary}--\r\n"
        )
        .as_bytes(),
    );
    assert_eq!(request.body, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn form_field_names_are_escaped() {
    let last_request: Arc<Mutex<Option<Request>>> = Arc::new(Mutex::new(None));
    let server_request = last_request.clone();
    let address = serve(move |request: Request| {
        *server_request.lock().unwrap() = Some(request);
        response(200, r#"{"Code":1000}"#)
    })
    .await;

    api(&address)
        .post("drive/blocks")
        .multipart(Multipart::new().file("Block\"\r\nX: y", "a\"b\nc", vec![1u8]))
        .send_unit()
        .await
        .unwrap();

    let request = last_request.lock().unwrap().take().unwrap();
    let body = String::from_utf8_lossy(&request.body);
    assert!(
        body.contains("name=\"Block%22%0D%0AX: y\"; filename=\"a%22b%0Ac\"\r\n"),
        "{body}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_invalid_content_type() {
    let address = serve(|_: Request| response(500, r#"{"Code":500}"#)).await;

    let result = api(&address)
        .post("mail/v4/attachments")
        .multipart(Multipart::new().part("Data", Part::bytes(vec![1u8]).content_type("a\r\nb")))
        .send_unit()
        .await;

    assert!(matches!(result, Err(Error::InvalidForm(_))), "{result:?}");
}
