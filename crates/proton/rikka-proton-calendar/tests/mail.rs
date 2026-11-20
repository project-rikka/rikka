use jiff::Timestamp;
use jiff::civil::date;
use rikka_proton_calendar::{
    Attendee, DateValue, Event, InviteWording, ItipMail, Method, Organizer, ParticipationStatus,
    reply_calendar, request_calendar,
};
use rikka_proton_mail::MessageId;
use serde_json::{Value, json};

use crate::support::signed_in;
use rikka_test_proton::{EMAIL, Record, Request, json_response, new_key, not_found};

const PROD_ID: &str = "-//Proton AG//test 1.0//EN";

fn paris(text: &str) -> DateValue {
    DateValue::parse(text, Some("Europe/Paris")).unwrap()
}

fn planning() -> Event {
    Event::new("mail@proton.me", paris("20240610T090000"))
        .with_end(paris("20240610T100000"))
        .with_summary("Planning")
        .with_location("Room 4")
        .with_description("Bring notes")
        .with_organizer(Organizer::new(EMAIL))
        .with_attendee(Attendee::new("ann@example.com"))
}

const STARTS: &str = "Monday June 10th, 2024 at 9:00 AM (GMT+2)";
const ENDS: &str = "Monday June 10th, 2024 at 10:00 AM (GMT+2)";

#[test]
fn invitation_mails_match_web() {
    let all_day = Event::new("day@proton.me", DateValue::Date(date(2024, 6, 10)))
        .with_end(DateValue::Date(date(2024, 6, 11)))
        .with_summary("Holiday");
    let long = Event::new("days@proton.me", DateValue::Date(date(2024, 6, 10)))
        .with_end(DateValue::Date(date(2024, 6, 12)));
    let moved = planning().with_location("Room 5");
    let unplaced = {
        let mut event = planning();
        event.component_mut().remove("LOCATION");
        event
    };

    let cases: Vec<(&str, ItipMail, &str, String)> = vec![
        (
            "new invitation",
            ItipMail::for_event(Method::Request, &planning()).creating().build().unwrap(),
            "Invitation for an event starting on Monday June 10th, 2024 at 9:00 AM (GMT+2)",
            format!(
                "You are invited to Planning.\n\nTIME:\n{STARTS} - {ENDS}\n\nLOCATION:\nRoom 4\n\nDESCRIPTION:\nBring notes"
            ),
        ),
        (
            "all-day invitation",
            ItipMail::for_event(Method::Request, &all_day).creating().build().unwrap(),
            "Invitation for an event on Monday June 10th, 2024",
            "You are invited to Holiday.\n\nTIME:\nMonday June 10th, 2024 (all day)".to_owned(),
        ),
        (
            "untitled days",
            ItipMail::for_event(Method::Request, &long).creating().build().unwrap(),
            "Invitation for an event starting on Monday June 10th, 2024",
            "You are invited to (no title).\n\nTIME:\nMonday June 10th, 2024 - Tuesday June 11th, 2024".to_owned(),
        ),
        (
            "update",
            ItipMail::for_event(Method::Request, &moved)
                .previous(&planning())
                .build()
                .unwrap(),
            "Update for an event starting on Monday June 10th, 2024 at 9:00 AM (GMT+2)",
            "This event was updated.\n\nHere's what changed:\n\nLOCATION:\nRoom 5".to_owned(),
        ),
        (
            "removed field",
            ItipMail::for_event(Method::Request, &unplaced)
                .previous(&planning())
                .build()
                .unwrap(),
            "Update for an event starting on Monday June 10th, 2024 at 9:00 AM (GMT+2)",
            "This event was updated.\n\nHere's what changed:\n\nLOCATION:\nRemoved".to_owned(),
        ),
        (
            "cancellation",
            ItipMail::for_event(Method::Cancel, &planning()).build().unwrap(),
            "Cancellation of an event starting on Monday June 10th, 2024 at 9:00 AM (GMT+2)",
            "Planning was canceled.".to_owned(),
        ),
        (
            "reply",
            ItipMail::for_event(Method::Reply, &planning())
                .reply_from("ann@example.com", ParticipationStatus::Accepted)
                .build()
                .unwrap(),
            "Re: Invitation for an event starting on Monday June 10th, 2024 at 9:00 AM (GMT+2)",
            "ann@example.com accepted your invitation to Planning".to_owned(),
        ),
    ];

    for (name, mail, subject, body) in cases {
        assert_eq!(mail.subject, subject, "{name}");
        assert_eq!(mail.body, body, "{name}");
    }

    let german = InviteWording {
        invitation_starting: "Einladung zu einem Termin ab {date}".to_owned(),
        invited: "Sie sind eingeladen zu".to_owned(),
        ..InviteWording::default()
    };
    let localized = ItipMail::for_event(Method::Request, &planning())
        .creating()
        .wording(&german)
        .build()
        .unwrap();
    assert!(
        localized
            .subject
            .starts_with("Einladung zu einem Termin ab Monday")
    );
    assert!(
        localized
            .body
            .starts_with("Sie sind eingeladen zu Planning.")
    );

    assert!(
        ItipMail::for_event(Method::Reply, &planning())
            .reply_from("ann@example.com", ParticipationStatus::NeedsAction)
            .build()
            .is_err()
    );
}

#[tokio::test]
async fn invitations_attach_signed_invite_ics() {
    let record = Record::default();
    let log = record.clone();
    let pgp = rikka_proton_pgp::Pgp::new();
    let signed = signed_in(new_key(&pgp, EMAIL), move |request: Request| {
        log.push(&request);
        match request.route() {
            ("GET", "/mail/v4/settings") => json_response(
                200,
                &json!({"Code": 1000, "MailSettings": {"Sign": 1, "AttachPublicKey": 1, "PGPScheme": 16}}),
            ),
            ("GET", "/core/v4/keys/all") => json_response(
                200,
                &json!({"Code": 1000, "Address": {"Keys": []}, "ProtonMX": false}),
            ),
            ("POST", "/mail/v4/messages/send/direct") => {
                json_response(200, &json!({"Code": 1000, "DeliveryTime": 1}))
            }
            _ => not_found(),
        }
    })
    .await;
    let at: Timestamp = "2024-06-01T00:00:00Z".parse().unwrap();
    let from = rikka_proton_account::AddressId::from(rikka_test_proton::ADDRESS_ID);

    let request = request_calendar(&planning(), PROD_ID, Vec::new(), at);
    let content = ItipMail::for_event(Method::Request, &planning())
        .creating()
        .build()
        .unwrap();
    signed
        .client
        .send_itip(&signed.mail, &from, &request)
        .to("ann@example.com")
        .content(&content)
        .send()
        .await
        .unwrap();

    let reply = reply_calendar(
        &planning(),
        "ann@example.com",
        ParticipationStatus::Accepted,
        PROD_ID,
        Vec::new(),
        at,
    )
    .unwrap();
    let parent = MessageId::from("invitation-message");
    signed
        .client
        .send_itip(&signed.mail, &from, &reply)
        .to("organizer@example.com")
        .in_reply_to(&parent)
        .send()
        .await
        .unwrap();

    let sent: Vec<Value> = record
        .requests()
        .iter()
        .filter(|request| request.path == "/mail/v4/messages/send/direct")
        .map(Request::json)
        .collect();
    assert_eq!(sent.len(), 2);

    let invitation = &sent[0];
    assert_eq!(invitation["Message"]["Subject"], content.subject);
    assert_eq!(invitation["Message"]["MIMEType"], "text/plain");
    assert_eq!(
        invitation["Message"]["Attachments"][0]["Filename"],
        "invite.ics"
    );
    assert_eq!(
        invitation["Message"]["Attachments"][0]["MIMEType"],
        "text/calendar; method=REQUEST"
    );
    assert_eq!(invitation["AutoSaveContacts"], 1);
    assert!(invitation.get("ParentID").is_none());

    let response = &sent[1];
    assert_eq!(
        response["Message"]["Attachments"][0]["MIMEType"],
        "text/calendar; method=REPLY"
    );
    assert_eq!(response["AutoSaveContacts"], 0);
    assert_eq!(response["ParentID"], "invitation-message");
    assert_eq!(response["Action"], 0);

    for sent in &sent {
        assert_eq!(
            sent["Message"]["Attachments"].as_array().map(Vec::len),
            Some(1)
        );
        let package = &sent["Packages"][0];
        assert_eq!(sent["Packages"].as_array().map(Vec::len), Some(1));
        assert_eq!(package["MIMEType"], "multipart/mixed");
        assert_eq!(package["Type"], 32);
        let address = package["Addresses"].as_object().unwrap();
        assert_eq!(address.values().next().unwrap()["Signature"], 1);

        let key =
            rikka_proton_api::base64::decode(package["BodyKey"]["Key"].as_str().unwrap()).unwrap();
        let data = rikka_proton_api::base64::decode(package["Body"].as_str().unwrap()).unwrap();
        let decrypted = pgp
            .decrypt()
            .with_session_key(rikka_proton_pgp::SessionKey::from_bytes(key).unwrap())
            .message(data)
            .unwrap();
        let mime = rikka_proton_mail::MimeMessage::parse(decrypted.data()).content();
        assert_eq!(mime.attachments()[0].name(), "invite.ics");
        assert_eq!(mime.attachments()[0].mime_type(), "text/calendar");
        assert!(
            String::from_utf8_lossy(mime.attachments()[0].bytes()).starts_with("BEGIN:VCALENDAR")
        );
    }
}
