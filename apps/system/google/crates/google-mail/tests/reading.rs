use google_test_server::Response;
use serde_json::json;

use crate::support::{Google, data, header};

fn thread() -> serde_json::Value {
    let shift_jis = [0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea];
    json!({
        "id": "thread",
        "historyId": "42",
        "messages": [{
            "id": "first",
            "threadId": "thread",
            "labelIds": ["INBOX", "UNREAD"],
            "snippet": "Tom &amp; Jerry",
            "internalDate": "1700000000000",
            "payload": {
                "mimeType": "multipart/mixed",
                "headers": [
                    header("From", "\"Sato, Taro\" <taro@example.com>"),
                    header("To", "rikka@example.com, Other <other@example.com>"),
                    header("Subject", "=?UTF-8?B?5a6a5L6L5Lya?="),
                    header("Message-ID", "<first@example.com>"),
                    header("References", "<root@example.com> <older@example.com>"),
                ],
                "parts": [
                    {
                        "mimeType": "multipart/alternative",
                        "parts": [
                            { "mimeType": "text/plain", "body": { "data": data(b"plain body") } },
                            {
                                "mimeType": "text/html",
                                "body": { "data": data(
                                    b"<h1>Hello</h1><p>See <a href=\"https://example.com/a\">this</a></p>\
                                      <img src=\"cid:logo@example\" alt=\"Logo\">\
                                      <img src=\"https://tracker.example/p.gif\" width=\"1\">\
                                      <img src=\"https://cdn.example/chart.png\" alt=\"Chart\">",
                                ) },
                            },
                        ],
                    },
                    {
                        "mimeType": "image/png",
                        "filename": "logo.png",
                        "headers": [
                            header("Content-ID", "<logo@example>"),
                            header("Content-Disposition", "inline; filename=\"logo.png\""),
                        ],
                        "body": { "attachmentId": "att-logo", "size": 120 },
                    },
                    {
                        "mimeType": "application/pdf",
                        "filename": "=?UTF-8?B?6LOH5paZ?=.pdf",
                        "body": { "attachmentId": "att-pdf", "size": 2048 },
                    },
                ],
            },
        }, {
            "id": "second",
            "threadId": "thread",
            "labelIds": ["SENT"],
            "internalDate": "1700000100000",
            "payload": {
                "mimeType": "text/plain",
                "headers": [
                    header("From", "rikka@example.com"),
                    header("Content-Type", "text/plain; charset=\"Shift_JIS\""),
                ],
                "body": { "data": data(&shift_jis) },
            },
        }, {
            "id": "third",
            "threadId": "thread",
            "internalDate": "1700000200000",
            "payload": {
                "mimeType": "text/plain",
                "headers": [header("From", "taro@example.com")],
                "body": { "attachmentId": "big-body", "size": 9 },
            },
        }],
    })
}

async fn opened() -> google_mail::Thread {
    let google = Google::start(|request| match request.path() {
        "/gmail/v1/users/me/threads/thread" => Response::json(&thread().to_string()),
        "/gmail/v1/users/me/messages/third/attachments/big-body" => {
            Response::json(&json!({ "size": 9, "data": data(b"late body") }).to_string())
        }
        _ => Response::status(404),
    });
    google
        .mail()
        .await
        .thread("thread")
        .await
        .expect("a thread")
}

#[tokio::test]
async fn full_thread_parses_headers_bodies_and_files() {
    let thread = opened().await;

    assert_eq!((thread.id.as_str(), thread.history_id), ("thread", 42));
    let first = &thread.messages[0];
    assert_eq!(first.from.name, "Sato, Taro");
    assert_eq!(first.to.len(), 2);
    assert_eq!(first.subject, "定例会");
    assert_eq!(first.snippet, "Tom & Jerry");
    assert_eq!(first.message_id, "<first@example.com>");
    assert_eq!(first.references.len(), 2);
    assert_eq!(first.text.as_deref(), Some("plain body"));
    assert!(first.has("UNREAD") && !first.is_draft());
    let names: Vec<(&str, bool)> = first
        .attachments
        .iter()
        .map(|attachment| (attachment.name.as_str(), attachment.inline))
        .collect();
    assert_eq!(names, [("logo.png", true), ("資料.pdf", false)]);
    assert_eq!(first.files().count(), 1);
    assert_eq!(
        first
            .inline_picture("cid:logo@example")
            .map(|found| found.attachment_id.as_str()),
        Some("att-logo")
    );
    assert_eq!(thread.messages[1].text.as_deref(), Some("日本語"));
    assert_eq!(thread.messages[2].text.as_deref(), Some("late body"));
}

#[tokio::test]
async fn html_reads_as_markdown_with_located_pictures() {
    let thread = opened().await;
    let first = &thread.messages[0];

    let mut sources = Vec::new();
    let blocked = first.readable(|source| {
        sources.push(source.to_owned());
        source
            .starts_with("cid:")
            .then(|| "/cache/logo.png".to_owned())
    });

    assert_eq!(
        blocked,
        "# Hello\n\nSee [this](<https://example.com/a>)\n\n![Logo](</cache/logo.png>)\n\nChart"
    );
    assert_eq!(
        sources,
        ["cid:logo@example", "https://cdn.example/chart.png"]
    );
    assert_eq!(first.quotable(), "plain body");
}

#[tokio::test]
async fn plain_text_keeps_quotes_and_links() {
    let google = Google::start(|_| {
        Response::json(
            &json!({
                "id": "thread",
                "messages": [{
                    "id": "only",
                    "payload": {
                        "mimeType": "text/plain",
                        "headers": [header("From", "taro@example.com")],
                        "body": { "data": data(
                            b"Sure, see https://example.com/doc?id=1.\nSecond *line*\n\n> earlier\n>> oldest\n",
                        ) },
                    },
                }],
            })
            .to_string(),
        )
    });
    let thread = google
        .mail()
        .await
        .thread("thread")
        .await
        .expect("a thread");

    assert_eq!(
        thread.messages[0].readable(|_| None),
        "Sure, see [https://example.com/doc?id=1](<https://example.com/doc?id=1>).\\\nSecond \\*line\\*\n\n\
         > earlier\n>\n> > oldest"
    );
}

#[test]
fn only_pictures_are_kept_for_display() {
    let folder = tempfile::tempdir().unwrap();
    let pictures = google_mail::Pictures::new(folder.path().join("pictures"));

    assert_eq!(pictures.kept("cid:logo"), None);
    let kept = pictures
        .keep("cid:logo", b"\x89PNG\r\n\x1a\npixels")
        .expect("a kept picture")
        .expect("a picture");
    let refused = pictures
        .keep("https://example.com/track", b"<html>not a picture</html>")
        .expect("no failure");

    assert!(kept.ends_with(".png") && !kept.contains('/'));
    assert_eq!(pictures.kept("cid:logo"), Some(kept.clone()));
    assert!(pictures.folder().join(kept).is_file());
    assert_eq!(refused, None);
    assert_eq!(pictures.kept("https://example.com/track"), None);
}
