use std::sync::{Arc, Mutex};

use rikka_proton_mail::{
    AutoReplyRepeat, AutoResponder, ImageProxy, MailSetting, MimeType, MobileSettings,
    NextMessageOnMove, PgpScheme, PmSignature, SpamAction, SwipeAction, ToolbarAction, ViewMode,
};
use serde_json::{Value, json};

use crate::support::{signed_in, signed_in_with};
use rikka_test_proton::{Request, not_found, response};

fn settings_json() -> Value {
    json!({
        "DisplayName": "Tess",
        "Signature": "<div>— Tess</div>",
        "AutoResponder": {
            "StartTime": 10, "EndTime": 20, "Repeat": 2, "DaysSelected": [1, 3],
            "Subject": "Away", "Message": "back soon", "IsEnabled": true, "Zone": "Asia/Tokyo"
        },
        "AutoSaveContacts": 0,
        "ViewMode": 1,
        "SwipeLeft": -1,
        "SwipeRight": 4,
        "PMSignature": 2,
        "PMSignatureReferralLink": null,
        "ImageProxy": 2,
        "Sign": 1,
        "PGPScheme": 8,
        "DraftMIMEType": "text/plain",
        "StickyLabels": 1,
        "DelaySendSeconds": 20,
        "FontFace": null,
        "FontSize": 14,
        "SpamAction": 1,
        "BlockSenderConfirmation": 1,
        "AutoDeleteSpamAndTrashDays": 30,
        "NextMessageOnMove": 2,
        "RemoveImageMetadata": true,
        "MailCategoryView": false,
        "MobileSettings": {
            "ListToolbar": {"IsCustom": true, "Actions": ["toggle_read", "trash", "snooze"]},
            "MessageToolbar": {"IsCustom": false, "Actions": []},
            "ConversationToolbar": {"IsCustom": false, "Actions": []}
        },
        "SomethingNew": 7
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn mail_settings_parse() {
    let signed = signed_in_with(settings_json(), |_: Request| not_found()).await;

    let settings = signed.client.mail_settings().await.unwrap();

    assert_eq!(settings.display_name(), "Tess");
    assert_eq!(settings.auto_responder().repeat(), AutoReplyRepeat::Weekly);
    assert_eq!(settings.auto_responder().days(), &[1, 3]);
    assert!(settings.auto_responder().is_enabled());
    assert!(!settings.auto_save_contacts());
    assert_eq!(settings.view_mode(), ViewMode::Messages);
    assert_eq!(settings.swipe_left(), SwipeAction::None);
    assert_eq!(settings.swipe_right(), SwipeAction::MarkRead);
    assert_eq!(settings.pm_signature(), PmSignature::Locked);
    assert!(!settings.pm_signature_referral());
    assert_eq!(settings.image_proxy(), ImageProxy::Proxy);
    assert!(settings.signs());
    assert_eq!(settings.pgp_scheme(), PgpScheme::Inline);
    assert_eq!(settings.draft_mime_type(), &MimeType::PlainText);
    assert!(settings.sticky_labels());
    assert_eq!(settings.delay_send_seconds(), 20);
    assert_eq!(settings.font_face(), None);
    assert_eq!(settings.font_size(), Some(14));
    assert_eq!(settings.spam_action(), Some(SpamAction::SpamAndUnsubscribe));
    assert!(settings.skips_block_sender_confirmation());
    assert_eq!(settings.auto_delete_days(), Some(30));
    assert_eq!(settings.next_message_on_move(), NextMessageOnMove::Enabled);
    assert!(settings.removes_image_metadata());
    assert!(settings.confirms_links());
    assert_eq!(settings.page_size(), 50);

    let list = settings.mobile().unwrap().list();
    assert!(list.is_custom());
    assert_eq!(
        list.actions(),
        &[
            ToolbarAction::ToggleRead,
            ToolbarAction::Trash,
            ToolbarAction::Other("snooze".to_owned())
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn setting_changes_hit_endpoints() {
    let captured: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
    let noted = Arc::clone(&captured);
    let signed = signed_in(move |request: Request| {
        if request.method == "PUT" && request.path.starts_with("/mail/v4/settings/") {
            noted
                .lock()
                .unwrap()
                .push((request.path.clone(), request.json()));
            return response(200, r#"{"Code":1000}"#);
        }
        not_found()
    })
    .await;

    let responder = AutoResponder::new("Away", "<p>later</p>")
        .window(100, 200)
        .repeating(AutoReplyRepeat::Daily, [])
        .zone("Europe/Paris");
    let cases = [
        (
            MailSetting::DisplayName("Tess".to_owned()),
            "display",
            json!({"DisplayName": "Tess"}),
        ),
        (
            MailSetting::Signature("bye".to_owned()),
            "signature",
            json!({"Signature": "bye"}),
        ),
        (
            MailSetting::SwipeLeft(SwipeAction::Archive),
            "swipeleft",
            json!({"SwipeLeft": 3}),
        ),
        (
            MailSetting::SwipeRight(SwipeAction::None),
            "swiperight",
            json!({"SwipeRight": -1}),
        ),
        (
            MailSetting::AutoSaveContacts(true),
            "autocontacts",
            json!({"AutoSaveContacts": 1}),
        ),
        (
            MailSetting::HideRemoteImages(true),
            "hide-remote-images",
            json!({"HideRemoteImages": 1}),
        ),
        (
            MailSetting::HideEmbeddedImages(false),
            "hide-embedded-images",
            json!({"HideEmbeddedImages": 0}),
        ),
        (
            MailSetting::ImageProxy {
                proxy: ImageProxy::Incorporator,
                enabled: true,
            },
            "imageproxy",
            json!({"ImageProxy": 1, "Action": 1}),
        ),
        (
            MailSetting::PmSignature(PmSignature::Disabled),
            "pmsignature",
            json!({"PMSignature": 0}),
        ),
        (MailSetting::Sign(true), "sign", json!({"Sign": 1})),
        (
            MailSetting::AttachPublicKey(true),
            "attachpublic",
            json!({"AttachPublicKey": 1}),
        ),
        (
            MailSetting::PgpScheme(PgpScheme::Mime),
            "pgpscheme",
            json!({"PGPScheme": 16}),
        ),
        (
            MailSetting::ViewMode(ViewMode::Conversations),
            "viewmode",
            json!({"ViewMode": 0}),
        ),
        (
            MailSetting::StickyLabels(true),
            "stickylabels",
            json!({"StickyLabels": 1}),
        ),
        (
            MailSetting::DraftMimeType(MimeType::PlainText),
            "drafttype",
            json!({"MIMEType": "text/plain"}),
        ),
        (
            MailSetting::AutoDeleteDays(None),
            "auto-delete-spam-and-trash-days",
            json!({"Days": 0}),
        ),
        (
            MailSetting::SpamAction(None),
            "spam-action",
            json!({"SpamAction": null}),
        ),
        (
            MailSetting::RemoveImageMetadata(true),
            "remove-image-metadata",
            json!({"RemoveImageMetadata": true}),
        ),
        (
            MailSetting::AutoResponder(responder.clone()),
            "autoresponder",
            json!({"AutoResponder": {
                "StartTime": 100, "EndTime": 200, "Repeat": 1, "DaysSelected": [],
                "Subject": "Away", "Message": "<p>later</p>", "IsEnabled": true, "Zone": "Europe/Paris"
            }}),
        ),
        (
            MailSetting::Mobile(MobileSettings::new(
                [ToolbarAction::Archive],
                [ToolbarAction::Reply, ToolbarAction::Forward],
                [],
            )),
            "mobilesettings",
            json!({"ListToolbar": ["archive"], "MessageToolbar": ["reply", "forward"], "ConversationToolbar": []}),
        ),
    ];

    let mut settings = signed.client.mail_settings().await.unwrap();
    for (setting, _, _) in &cases {
        signed.client.change_setting(setting).await.unwrap();
        settings.apply(setting);
    }
    let cached = signed.client.cached_mail_settings().await.unwrap();
    assert_eq!(cached, settings);
    let captured = captured.lock().unwrap();
    for ((_, path, body), (sent_path, sent_body)) in cases.iter().zip(captured.iter()) {
        assert_eq!(sent_path, &format!("/mail/v4/settings/{path}"));
        assert_eq!(sent_body, body, "{path}");
    }
    assert_eq!(captured.len(), cases.len());

    assert_eq!(settings.signature(), "bye");
    assert_eq!(settings.swipe_left(), SwipeAction::Archive);
    assert_eq!(settings.image_proxy(), ImageProxy::All);
    assert_eq!(settings.auto_responder(), &responder);
    assert_eq!(
        settings
            .mobile()
            .map(|mobile| mobile.message().actions().len()),
        Some(2)
    );
}
