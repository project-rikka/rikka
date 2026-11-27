use google_mail::{Address, Error, Mailbox, Outgoing, Query};
use google_test_server::Response;
use serde_json::{Value, json};

use crate::support::{Google, metadata};

#[tokio::test]
async fn listing_summarizes_each_thread_in_order() {
    let google = Google::start(|request| {
        match request.path() {
        "/gmail/v1/users/me/threads" => Response::json(
            &json!({
                "threads": [{ "id": "a" }, { "id": "gone" }, { "id": "b" }],
                "nextPageToken": "more",
            })
            .to_string(),
        ),
        "/gmail/v1/users/me/threads/a" => Response::json(
            &json!({ "id": "a", "historyId": "9", "messages": [
                metadata("a1", "Taro <taro@example.com>", "=?UTF-8?B?5a6a5L6L5Lya?=", &["INBOX", "UNREAD"]),
                metadata("a2", "rikka@example.com", "Re: later", &["SENT", "STARRED"]),
                metadata("a3", "Taro <taro@example.com>", "Re: later", &["INBOX"]),
            ] })
            .to_string(),
        ),
        "/gmail/v1/users/me/threads/b" => Response::json(
            &json!({ "id": "b", "messages": [metadata("b1", "b@example.com", "Hello", &[])] })
                .to_string(),
        ),
        _ => Response::status(404),
    }
    });
    let mail = google.mail().await;
    let query = Query {
        label: Mailbox::Inbox.label().map(str::to_owned),
        search: " from:taro ".to_owned(),
    };

    let page = mail.threads(&query, Some("page-2")).await.expect("a page");

    assert_eq!(page.next.as_deref(), Some("more"));
    let ids: Vec<&str> = page
        .threads
        .iter()
        .map(|thread| thread.id.as_str())
        .collect();
    assert_eq!(ids, ["a", "b"]);
    let first = &page.threads[0];
    assert_eq!(first.subject, "定例会");
    assert_eq!(first.snippet, "It's here");
    assert_eq!(first.messages, 3);
    assert_eq!(first.history_id, 9);
    assert_eq!(first.date_ms, 1_700_000_000_000);
    assert_eq!(first.senders.len(), 2);
    assert!(first.is_unread() && first.is_starred() && first.has_attachment);
    let listing = &google.account.asked("/gmail/v1/users/me/threads")[0].query();
    assert_eq!(listing["labelIds"], "INBOX");
    assert_eq!(listing["q"], "from:taro");
    assert_eq!(listing["pageToken"], "page-2");
    let detail = &google.account.asked("/gmail/v1/users/me/threads/a")[0];
    assert_eq!(detail.query()["format"], "metadata");
    assert!(
        detail
            .target
            .contains("metadataHeaders=From&metadataHeaders=Subject")
    );
}

#[tokio::test]
async fn thread_actions_send_label_changes() {
    let google = Google::start(|_| Response::json("{}"));
    let mail = google.mail().await;

    mail.relabel("t", &["STARRED"], &["UNREAD", "INBOX"])
        .await
        .expect("relabelled");
    mail.trash("t").await.expect("trashed");
    mail.untrash("t").await.expect("restored");

    let calls = google.account.calls();
    assert_eq!(calls[0].path(), "/gmail/v1/users/me/threads/t/modify");
    assert_eq!(
        serde_json::from_slice::<Value>(&calls[0].body).expect("json"),
        json!({ "addLabelIds": ["STARRED"], "removeLabelIds": ["UNREAD", "INBOX"] })
    );
    assert_eq!(
        (calls[1].method.as_str(), calls[1].path()),
        ("POST", "/gmail/v1/users/me/threads/t/trash")
    );
    assert_eq!(calls[2].path(), "/gmail/v1/users/me/threads/t/untrash");
}

#[tokio::test]
async fn sending_uploads_the_message_into_its_thread() {
    let google = Google::start(|request| match request.path() {
        "/upload/gmail/v1/users/me/messages/send" => Response::json(r#"{"id":"sent-1"}"#),
        "/gmail/v1/users/me/drafts" => Response::json(r#"{"id":"draft-1","message":{"id":"m-1"}}"#),
        _ => Response::status(404),
    });
    let mail = google.mail().await;
    let mut outgoing = Outgoing {
        from: Address::new("Rikka", "rikka@example.com"),
        to: vec![Address::new("", "taro@example.com")],
        subject: "Hello".to_owned(),
        text: "Body".to_owned(),
        ..Outgoing::default()
    };

    let sent = mail.send(&outgoing, Some("thread-9")).await.expect("sent");
    outgoing.to.clear();
    let draft = mail
        .save_draft(&outgoing, None, None)
        .await
        .expect("a draft");

    assert_eq!(sent, "sent-1");
    let upload = &google
        .account
        .asked("/upload/gmail/v1/users/me/messages/send")[0];
    assert_eq!(upload.query()["uploadType"], "multipart");
    assert!(
        upload
            .header("content-type")
            .unwrap_or_default()
            .starts_with("multipart/related; boundary=")
    );
    let body = upload.text();
    assert!(body.contains("{\"threadId\":\"thread-9\"}"));
    assert!(body.contains("Content-Type: message/rfc822\r\n\r\nFrom: Rikka <rikka@example.com>\r\nTo: taro@example.com\r\nSubject: Hello\r\n"));
    assert_eq!(
        (draft.id.as_str(), draft.message_id.as_str()),
        ("draft-1", "m-1")
    );
    let saved: Value =
        serde_json::from_slice(&google.account.asked("/gmail/v1/users/me/drafts")[0].body)
            .expect("json");
    assert!(saved["message"]["raw"].is_string());
    assert!(saved["message"].get("threadId").is_none());
}

#[tokio::test]
async fn activity_names_touched_threads_until_history_expires() {
    let google = Google::start(|request| {
        let query = request.query();
        match (query["startHistoryId"].as_str(), query.get("pageToken")) {
            ("100", None) => Response::json(
                &json!({
                    "nextPageToken": "next",
                    "history": [
                        { "messages": [{ "id": "m1", "threadId": "t1" }] },
                        { "messages": [{ "id": "m2", "threadId": "t2" }, { "id": "m3", "threadId": "t1" }] },
                    ],
                })
                .to_string(),
            ),
            ("100", Some(_)) => Response::json(
                &json!({ "historyId": "150", "history": [{ "messages": [{ "id": "m4", "threadId": "t3" }] }] })
                    .to_string(),
            ),
            _ => Response::status(404),
        }
    });
    let mail = google.mail().await;

    let activity = mail.activity(100).await.expect("activity");
    let expired = mail.activity(1).await;

    assert_eq!(activity.threads, ["t1", "t2", "t3"]);
    assert_eq!(activity.history_id, 150);
    assert!(matches!(expired, Err(Error::HistoryExpired)));
}
