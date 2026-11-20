use rikka_net::{Client, Form, Part};

use crate::support::{Request, response, spawn};
use tempfile::TempDir;

const CONTENTS: &[u8] = b"the quick brown fox jumps over the lazy dog";

fn fixture(directory: &TempDir, name: &str) -> std::path::PathBuf {
    let path = directory.path().join(name);
    std::fs::write(&path, CONTENTS).expect("the fixture is written");
    path
}

#[tokio::test]
async fn uploads_file_as_multipart_field() {
    let url = spawn(|request: Request| {
        let content_type = request
            .header("content-type")
            .expect("a content type is sent");
        assert!(
            content_type.starts_with("multipart/form-data; boundary="),
            "{content_type}"
        );

        let body = request.text();
        assert!(
            body.contains(
                "Content-Disposition: form-data; name=\"document\"; filename=\"notes.txt\""
            ),
            "{body}"
        );
        assert!(body.contains("Content-Type: text/plain"), "{body}");
        assert!(body.contains("the quick brown fox"), "{body}");

        Some(response(200, &[], b""))
    })
    .await;

    let directory = TempDir::new().expect("a temporary directory");
    let source = fixture(&directory, "notes.txt");

    let status = Client::new()
        .upload(&url, &source)
        .field("document")
        .send()
        .await
        .expect("the upload succeeds")
        .status();

    assert!(status.is_success());
}

#[tokio::test]
async fn escapes_special_file_name() {
    let url = spawn(|request: Request| {
        let body = request.text();
        assert!(body.contains("filename=\"od%22d.txt\""), "{body}");
        Some(response(200, &[], b""))
    })
    .await;

    let form = Form::new().part(
        "file",
        Part::bytes(CONTENTS.to_vec()).file_name("od\"d.txt"),
    );

    Client::new()
        .post(&url)
        .multipart(form)
        .send()
        .await
        .expect("the upload succeeds");
}
