use std::sync::Arc;

use rikka_proton_pgp::{armor_message, unarmor};
use serde_json::{Value, json};

use rikka_test_proton::{
    EMAIL, FakeSrp, Request, ResponseFuture, SIGNED_MODULUS, json_response, response, success,
};

use crate::fixture::{Fixture, NOW};
use crate::ids::{
    BOOKMARKED_NAME, BOOKMARKED_PASSWORD, FOREIGN_OWNER, INVITATION, INVITEE, PUBLIC_URL, ROOT,
    SHARE, SHARED_NAME, TOKEN, VOLUME,
};
use crate::state::{DriveState, text};

const BOOKMARK_SALT: [u8; 16] = *b"0123456789abcdef";
const SHARE_URL: &str = "POST /drive/shares/share/urls";

pub(crate) fn respond(
    state: &mut DriveState,
    srp: &mut Option<FakeSrp>,
    request: &Request,
) -> Option<ResponseFuture> {
    let fixture = Arc::clone(&state.fixture);
    let fixture = &*fixture;
    let (method, path) = request.route();
    Some(match (method, path) {
        ("GET", "/core/v4/keys/all") => published(fixture, request),
        ("POST", "/drive/v2/shares/share/invitations") => {
            let body = request.json();
            let sent = &body["Invitation"];
            json_response(
                200,
                &json!({
                    "Code": 1000,
                    "Invitation": {
                        "InvitationID": INVITATION,
                        "InviterEmail": sent["InviterEmail"],
                        "InviteeEmail": sent["InviteeEmail"],
                        "Permissions": sent["Permissions"],
                        "KeyPacket": sent["KeyPacket"],
                        "KeyPacketSignature": sent["KeyPacketSignature"],
                        "CreateTime": NOW,
                    },
                }),
            )
        }
        ("POST", "/drive/v2/shares/share/external-invitations") => {
            let body = request.json();
            let sent = &body["ExternalInvitation"];
            json_response(
                200,
                &json!({
                    "Code": 1000,
                    "ExternalInvitation": {
                        "ExternalInvitationID": "external",
                        "InviterEmail": EMAIL,
                        "InviteeEmail": sent["InviteeEmail"],
                        "Permissions": sent["Permissions"],
                        "ExternalInvitationSignature": sent["ExternalInvitationSignature"],
                        "State": 1,
                        "CreateTime": NOW,
                    },
                }),
            )
        }
        ("GET", "/drive/v2/shares/share/invitations") => {
            let (packet, signature) = fixture
                .invitation_packet(&fixture.invitee_key.public_key(), &fixture.owner.address);
            let (_, forged) =
                fixture.invitation_packet(&fixture.invitee_key.public_key(), &fixture.foreign_key);
            json_response(
                200,
                &json!({
                    "Code": 1000,
                    "Invitations": [
                        {"InvitationID": "honest", "InviterEmail": EMAIL, "InviteeEmail": INVITEE, "Permissions": 4, "KeyPacket": packet, "KeyPacketSignature": signature, "CreateTime": NOW},
                        {"InvitationID": "forged", "InviterEmail": EMAIL, "InviteeEmail": INVITEE, "Permissions": 6, "KeyPacket": packet, "KeyPacketSignature": forged, "CreateTime": NOW},
                    ],
                }),
            )
        }
        ("GET", "/drive/v2/shares/share/members") => {
            let (packet, signature) = fixture
                .invitation_packet(&fixture.invitee_key.public_key(), &fixture.owner.address);
            json_response(
                200,
                &json!({
                    "Code": 1000,
                    "Members": [{
                        "MemberID": "member",
                        "InviterEmail": EMAIL,
                        "Email": INVITEE,
                        "Permissions": 22,
                        "KeyPacket": packet,
                        "KeyPacketSignature": signature,
                        "SessionKeySignature": "",
                        "CreateTime": NOW,
                    }],
                }),
            )
        }
        ("PUT", "/drive/v2/shares/share/members/member")
        | ("DELETE", "/drive/v2/shares/share/members/member")
        | ("POST", "/drive/v2/shares/invitations/invitation/reject")
        | ("POST", "/drive/v2/shares/invitations/invitation/accept")
        | ("DELETE", "/drive/shares/share")
        | ("DELETE", "/drive/shares/share/urls/url")
        | ("POST", "/drive/v2/urls/token/bookmark")
        | ("DELETE", "/drive/v2/urls/token/bookmark")
        | ("POST", "/drive/report/url") => success(),
        ("GET", "/drive/v2/shares/invitations") => match request.query("AnchorID") {
            None => json_response(
                200,
                &json!({
                    "Code": 1000,
                    "Invitations": [{"VolumeID": VOLUME, "ShareID": SHARE, "InvitationID": INVITATION, "ShareTargetType": 1}],
                    "AnchorID": "later",
                    "More": true,
                }),
            ),
            Some(_) => json_response(
                200,
                &json!({
                    "Code": 1000,
                    "Invitations": [{"VolumeID": VOLUME, "ShareID": "other", "InvitationID": "second", "ShareTargetType": 2}],
                    "AnchorID": null,
                    "More": false,
                }),
            ),
        },
        ("GET", "/drive/v2/shares/invitations/invitation") => received_invitation(fixture),
        ("POST", "/drive/volumes/volume/shares") => {
            let root = text(&request.json()["RootLinkID"]);
            state.mark_shared(&root);
            json_response(
                200,
                &json!({"Code": 1000, "Share": {"ID": SHARE, "EditorsCanShare": false}}),
            )
        }
        ("GET", "/drive/v2/sharedwithme") => match request.query("AnchorID") {
            None => json_response(
                200,
                &json!({
                    "Code": 1000,
                    "Links": [{"VolumeID": "their-volume", "ShareID": SHARE, "LinkID": ROOT, "ShareTargetType": 1}],
                    "AnchorID": "rest",
                    "More": true,
                }),
            ),
            Some(_) => json_response(
                200,
                &json!({
                    "Code": 1000,
                    "Links": [{"VolumeID": "their-volume", "ShareID": "album-share", "LinkID": "album", "ShareTargetType": 3}],
                    "AnchorID": null,
                    "More": false,
                }),
            ),
        },
        ("POST", "/drive/shares/share/urls") => {
            let body = request.json();
            *srp = Some(FakeSrp::registered(
                body["UrlPasswordSalt"].as_str().unwrap_or_default(),
                body["SRPVerifier"].as_str().unwrap_or_default(),
            ));
            json_response(200, &json!({"Code": 1000, "ShareURL": share_url(&body)}))
        }
        ("GET", "/drive/shares/share/urls") => {
            let urls: Vec<Value> = state
                .bodies(SHARE_URL)
                .last()
                .map(share_url)
                .into_iter()
                .collect();
            json_response(200, &json!({"Code": 1000, "ShareURLs": urls, "Links": {}}))
        }
        ("GET", "/drive/urls/token/info") => {
            let body = state.bodies(SHARE_URL).pop().unwrap_or_default();
            let Some(srp) = srp.as_mut() else {
                return Some(response(422, r#"{"Code":2501,"Error":"No such link"}"#));
            };
            json_response(
                200,
                &json!({
                    "Code": 1000,
                    "Modulus": SIGNED_MODULUS,
                    "ServerEphemeral": srp.challenge(),
                    "UrlPasswordSalt": body["UrlPasswordSalt"],
                    "SRPSession": "srp-session",
                    "Version": 4,
                    "Flags": body["Flags"],
                    "IsDoc": false,
                    "VendorType": 0,
                    "DirectAccess": null,
                }),
            )
        }
        ("POST", "/drive/urls/token/auth") => {
            let body = state.bodies(SHARE_URL).pop().unwrap_or_default();
            public_auth(fixture, srp.as_mut(), &body, request)
        }
        ("GET", "/drive/urls/token") => json_response(
            200,
            &json!({
                "Code": 1000,
                "Token": {
                    "Token": TOKEN,
                    "LinkType": 1,
                    "VolumeID": VOLUME,
                    "LinkID": ROOT,
                    "NodeKey": fixture.root.armored_key,
                    "NodePassphrase": fixture.root.armored_passphrase,
                    "Name": fixture.owner.name(&fixture.share.key.public_key(), "root"),
                    "MIMEType": "Folder",
                    "Permissions": 4,
                },
            }),
        ),
        ("GET", "/drive/v2/shared-bookmarks") => json_response(200, &bookmarks(fixture)),
        _ => return None,
    })
}

fn received_invitation(fixture: &Fixture) -> ResponseFuture {
    let (packet, signature) =
        fixture.invitation_packet(&fixture.owner.address.public_key(), &fixture.foreign_key);
    let name = fixture
        .pgp
        .encrypt()
        .to(fixture.share.key.public_key())
        .signed_by(fixture.foreign_key.clone())
        .armored_message(SHARED_NAME)
        .unwrap();
    json_response(
        200,
        &json!({
            "Code": 1000,
            "Invitation": {
                "InvitationID": INVITATION,
                "InviterEmail": FOREIGN_OWNER,
                "InviteeEmail": EMAIL,
                "Permissions": 6,
                "KeyPacket": packet,
                "KeyPacketSignature": signature,
                "CreateTime": NOW,
            },
            "Share": {
                "ShareID": SHARE,
                "VolumeID": VOLUME,
                "Passphrase": fixture.share.armored_passphrase,
                "ShareKey": fixture.share.armored_key,
                "CreatorEmail": FOREIGN_OWNER,
                "ShareTargetType": 1,
            },
            "Link": {"Type": 1, "LinkID": ROOT, "Name": name, "MIMEType": null},
        }),
    )
}

fn bookmarks(fixture: &Fixture) -> Value {
    let key_password = rikka_proton_pgp::Srp::new()
        .mailbox_passphrase(BOOKMARKED_PASSWORD, BOOKMARK_SALT)
        .unwrap();
    let mut message = fixture
        .pgp
        .encrypt()
        .with_password(key_password.as_bytes())
        .key_packets(&fixture.share_session_key())
        .unwrap();
    message.extend(unarmor(&fixture.share.armored_passphrase).unwrap());
    let encrypted_password = fixture
        .pgp
        .encrypt()
        .to(fixture.owner.address.public_key())
        .armored_message(BOOKMARKED_PASSWORD)
        .unwrap();

    json!({
        "Code": 1000,
        "Bookmarks": [
            {
                "EncryptedUrlPassword": encrypted_password,
                "CreateTime": NOW,
                "Token": {
                    "Token": TOKEN,
                    "LinkType": 1,
                    "VolumeID": VOLUME,
                    "LinkID": ROOT,
                    "SharePasswordSalt": rikka_proton_api::base64::encode(BOOKMARK_SALT),
                    "SharePassphrase": armor_message(message).unwrap(),
                    "ShareKey": fixture.share.armored_key,
                    "NodePassphrase": fixture.root.armored_passphrase,
                    "NodeKey": fixture.root.armored_key,
                    "Name": fixture.owner.name(&fixture.share.key.public_key(), BOOKMARKED_NAME),
                    "ContentKeyPacket": null,
                    "MIMEType": "Folder",
                    "Permissions": 4,
                },
            },
            {
                "EncryptedUrlPassword": null,
                "CreateTime": NOW,
                "Token": {
                    "Token": "locked",
                    "LinkType": 2,
                    "VolumeID": VOLUME,
                    "LinkID": "file",
                    "SharePasswordSalt": "",
                    "SharePassphrase": "",
                    "ShareKey": "",
                    "NodePassphrase": "",
                    "NodeKey": "",
                    "Name": "",
                    "MIMEType": "text/plain",
                    "Permissions": 4,
                },
            },
        ],
    })
}

fn published(fixture: &Fixture, request: &Request) -> ResponseFuture {
    let email = request
        .query("Email")
        .map(|email| email.replace("%40", "@"));
    let keys: Vec<Value> = match email.as_deref() {
        Some(INVITEE) => vec![armored_entry(&fixture.invitee_key.public_key())],
        Some(FOREIGN_OWNER) => vec![armored_entry(&fixture.foreign_key.public_key())],
        _ => Vec::new(),
    };

    json_response(
        200,
        &json!({"Code": 1000, "Address": {"Keys": keys}, "Warnings": [], "ProtonMX": false, "IsProton": 0}),
    )
}

fn share_url(body: &Value) -> Value {
    json!({
        "Token": TOKEN,
        "ShareURLID": "url",
        "ShareID": SHARE,
        "PublicUrl": PUBLIC_URL,
        "ExpirationTime": body["ExpirationTime"],
        "LastAccessTime": null,
        "CreateTime": NOW,
        "MaxAccesses": body["MaxAccesses"],
        "NumAccesses": 0,
        "CreatorEmail": EMAIL,
        "Permissions": body["Permissions"],
        "Flags": body["Flags"],
        "UrlPasswordSalt": body["UrlPasswordSalt"],
        "SharePasswordSalt": body["SharePasswordSalt"],
        "SRPVerifier": body["SRPVerifier"],
        "SRPModulusID": body["SRPModulusID"],
        "Password": body["Password"],
        "SharePassphraseKeyPacket": body["SharePassphraseKeyPacket"],
        "Name": null,
    })
}

fn public_auth(
    fixture: &Fixture,
    srp: Option<&mut FakeSrp>,
    body: &Value,
    request: &Request,
) -> ResponseFuture {
    let offered = request.json();
    let proof = srp.and_then(|srp| {
        srp.prove(
            offered["ClientEphemeral"].as_str().unwrap_or_default(),
            offered["ClientProof"].as_str().unwrap_or_default(),
        )
    });
    let Some(proof) = proof else {
        return response(422, r#"{"Code":8002,"Error":"Incorrect password"}"#);
    };

    let mut message = rikka_proton_api::base64::decode(
        body["SharePassphraseKeyPacket"]
            .as_str()
            .unwrap_or_default(),
    )
    .unwrap();
    message.extend(unarmor(&fixture.share.armored_passphrase).unwrap());

    json_response(
        200,
        &json!({
            "Code": 1000,
            "UID": "public-session",
            "AccessToken": "public-access",
            "ExpiresIn": 3600,
            "TokenType": "Bearer",
            "ServerProof": proof,
            "Share": {
                "VolumeID": VOLUME,
                "LinkID": ROOT,
                "SharePasswordSalt": body["SharePasswordSalt"],
                "SharePassphrase": armor_message(message).unwrap(),
                "ShareKey": fixture.share.armored_key,
                "PublicPermissions": 4,
            },
        }),
    )
}

fn armored_entry(key: &rikka_proton_pgp::PublicKey) -> Value {
    json!({
        "Flags": 3,
        "PublicKey": key.export_armored().unwrap(),
        "Source": 0,
    })
}
