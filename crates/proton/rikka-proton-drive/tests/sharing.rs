use rikka_proton_drive::{
    AbuseCategory, AbuseReport, InvitationId, Invited, Node, Role, ShareId, ShareTarget,
    ShareUrlFlags, ShareUrlSettings,
};
use rikka_proton_pgp::{LockedKey, VerificationContext};

use rikka_test_proton::EMAIL;
use rikka_test_proton_drive::{
    BOOKMARKED_NAME, BOOKMARKED_PASSWORD, DummyDrive, EXTERNAL_INVITEE, FOLDER_NAME, FOREIGN_OWNER,
    INVITATION, INVITEE, PUBLIC_URL, SHARED_NAME, Shape,
};

const INVITER: &str = "drive.share-member.inviter";
const MEMBER: &str = "drive.share-member.member";
const EXTERNAL: &str = "drive.share-member.external-invitation";

fn base64_decode(text: &str) -> Vec<u8> {
    rikka_proton_api::base64::decode(text).unwrap()
}

async fn share_keys(client: &rikka_proton_drive::DriveClient) -> rikka_proton_drive::ShareKeys {
    client.open_share(&ShareId::from("share")).await.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn public_link_opens_and_reports() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let share = share_keys(&client).await;

    let created = client
        .create_share_url(&share, ShareUrlSettings::new().custom_password("hunter2"))
        .await
        .unwrap();
    assert_eq!(created.flags(), ShareUrlFlags::GeneratedAndCustomPassword);
    assert_eq!(created.role(), Role::Viewer);
    assert_eq!(created.password().expose().len(), 12);
    assert_eq!(
        created.url(),
        format!("{PUBLIC_URL}#{}", created.password().expose())
    );

    let body = drive.last_body("POST /drive/shares/share/urls");
    assert_eq!(body["CreatorEmail"].as_str(), Some(EMAIL));
    assert_eq!(body["SRPModulusID"].as_str(), Some("modulus"));
    assert_eq!(body["Flags"].as_i64(), Some(3));
    let full_password = format!("{}hunter2", created.password().expose());
    let sent = drive
        .pgp()
        .decrypt()
        .with_key(drive.owner().address.clone())
        .message(body["Password"].as_str().unwrap())
        .unwrap();
    assert_eq!(sent.data(), full_password.as_bytes());

    let listed = client.share_urls(share.id()).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].password().expose(), created.password().expose());
    assert_eq!(
        listed[0].custom_password().map(|custom| custom.expose()),
        Some("hunter2")
    );

    let refused = client
        .open_public_link(&created.url(), Some("wrong"))
        .await
        .unwrap_err();
    assert!(
        matches!(refused, rikka_proton_drive::Error::Api(_)),
        "{refused}"
    );

    let link = client
        .open_public_link(&created.url(), Some("hunter2"))
        .await
        .unwrap();
    assert_eq!(link.name(), "root");
    assert_eq!(link.token(), "token");
    assert_eq!(link.role(), Role::Viewer);
    assert_eq!(
        link.share_key().fingerprint(),
        drive.share().key.fingerprint()
    );

    client
        .report_public_link(
            &link,
            AbuseReport::new(AbuseCategory::Spam)
                .reporter_email("reporter@proton.test")
                .message("unsolicited"),
        )
        .await
        .unwrap();
    let report = drive.last_body("POST /drive/report/url");
    assert_eq!(report["AbuseCategory"].as_str(), Some("spam"));
    assert_eq!(report["Password"].as_str(), Some(full_password.as_str()));
    assert_eq!(
        report["ResourcePassphrase"].as_str(),
        Some(drive.root().passphrase.as_str())
    );
    assert_eq!(report["LinkID"].as_str(), Some("root"));
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_past_expiration_and_foreign_url() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let share = share_keys(&client).await;

    let refused = client
        .create_share_url(
            &share,
            ShareUrlSettings::new().expires_at(jiff::Timestamp::from_second(1_000).unwrap()),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(refused, rikka_proton_drive::Error::ExpirationInPast),
        "{refused}"
    );

    let refused = client
        .open_public_link("https://drive.proton.me/files/abc", None)
        .await
        .unwrap_err();
    assert!(
        matches!(refused, rikka_proton_drive::Error::NotAPublicLink(_)),
        "{refused}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn invitations_wrap_share_key() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let share = share_keys(&client).await;

    let invited = client
        .invite(&share, INVITEE, Role::Editor)
        .message("have a look")
        .send()
        .await
        .unwrap();
    let Invited::Member(invitation) = invited else {
        panic!("expected internal invitation");
    };
    assert!(invitation.signature().is_valid());
    assert_eq!(invitation.role(), Role::Editor);

    let body = drive.last_body("POST /drive/v2/shares/share/invitations");
    let packet = base64_decode(body["Invitation"]["KeyPacket"].as_str().unwrap());
    let unwrapped = drive
        .pgp()
        .decrypt()
        .with_key(drive.invitee_key().clone())
        .session_key(&packet)
        .unwrap();
    assert_eq!(unwrapped.as_bytes(), drive.share_session_key().as_bytes());
    let signature = base64_decode(body["Invitation"]["KeyPacketSignature"].as_str().unwrap());
    assert!(
        drive
            .pgp()
            .verify()
            .with_key(drive.owner().address.public_key())
            .with_context(VerificationContext::new(INVITER))
            .detached(&packet, &signature)
            .is_valid()
    );
    assert_eq!(
        body["EmailDetails"]["Message"].as_str(),
        Some("have a look")
    );

    let invited = client
        .invite(&share, EXTERNAL_INVITEE, Role::Viewer)
        .send()
        .await
        .unwrap();
    let Invited::External(external) = invited else {
        panic!("expected external invitation");
    };
    assert!(external.signature().is_valid());
    let body = drive.last_body("POST /drive/v2/shares/share/external-invitations");
    let payload = format!(
        "{EXTERNAL_INVITEE}|{}",
        rikka_proton_api::base64::encode(drive.share_session_key().as_bytes())
    );
    let signature = base64_decode(
        body["ExternalInvitation"]["ExternalInvitationSignature"]
            .as_str()
            .unwrap(),
    );
    assert!(
        drive
            .pgp()
            .verify()
            .with_key(drive.owner().address.public_key())
            .with_context(VerificationContext::new(EXTERNAL))
            .detached(payload.as_bytes(), &signature)
            .is_valid()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn listings_verify_inviter() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let share = ShareId::from("share");

    let invitations = client.invitations(&share).await.unwrap();
    assert_eq!(invitations.len(), 2);
    assert!(invitations[0].signature().is_valid());
    assert!(
        !invitations[1].signature().is_valid(),
        "forged signature accepted"
    );

    let members = client.members(&share).await.unwrap();
    assert_eq!(members[0].email(), INVITEE);
    assert_eq!(members[0].role(), Role::Admin);
    assert!(members[0].signature().is_valid());
}

#[tokio::test(flavor = "multi_thread")]
async fn accept_invitation_signs_membership() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;

    let pending = client.pending_invitations().await.unwrap();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[1].target(), ShareTarget::File);

    let received = client
        .received_invitation(&InvitationId::from(INVITATION))
        .await
        .unwrap();
    assert_eq!(received.name(), SHARED_NAME);
    assert_eq!(received.creator(), FOREIGN_OWNER);
    assert!(received.invitation().signature().is_valid());

    client.accept_invitation(&received).await.unwrap();
    let signature = base64_decode(
        drive.last_body("POST /drive/v2/shares/invitations/invitation/accept")["SessionKeySignature"]
            .as_str()
            .unwrap(),
    );
    assert!(
        drive
            .pgp()
            .verify()
            .with_key(drive.owner().address.public_key())
            .with_context(VerificationContext::new(MEMBER))
            .detached(drive.share_session_key().as_bytes(), &signature)
            .is_valid()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn share_reencrypts_folder_session_keys() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let share = share_keys(&client).await;
    let root = client.root(&share).await.unwrap();
    let children = client.children(share.volume_id(), &root).await.unwrap();
    let folder = children[0].clone();
    let Node::Folder(_) = &folder else {
        panic!("expected folder as first child");
    };

    client.share_node(&share, &folder).await.unwrap();

    let body = drive.last_body("POST /drive/volumes/volume/shares");
    assert_eq!(body["RootLinkID"].as_str(), Some("folder"));
    let armored = body["SharePassphrase"].as_str().unwrap();
    let through_node = drive
        .pgp()
        .decrypt()
        .with_key(drive.folder().key.clone())
        .message(armored)
        .unwrap();
    let through_address = drive
        .pgp()
        .decrypt()
        .with_key(drive.owner().address.clone())
        .message(armored)
        .unwrap();
    assert_eq!(through_node.data(), through_address.data());
    let share_key = LockedKey::import(body["ShareKey"].as_str().unwrap())
        .unwrap()
        .unlock(through_node.data())
        .unwrap();
    let name_key = drive
        .pgp()
        .decrypt()
        .with_key(share_key)
        .session_key(base64_decode(body["NameKeyPacket"].as_str().unwrap()))
        .unwrap();
    let name = drive
        .pgp()
        .decrypt()
        .with_session_key(name_key)
        .message(folder.link().encrypted_name())
        .unwrap();
    assert_eq!(name.data(), FOLDER_NAME.as_bytes());

    client.unshare(&ShareId::from("share"), true).await.unwrap();
    assert_eq!(drive.request_count("DELETE /drive/shares/share"), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn foreign_share_opens_via_membership() {
    let drive = DummyDrive::with_shape(Shape::new().member_share()).await;
    let client = drive.client().await;

    let shared = client.shared_with_me().await.unwrap();
    assert_eq!(shared.len(), 2);
    assert_eq!(shared[1].target(), ShareTarget::Album);

    let share = client.open_share(shared[0].share_id()).await.unwrap();
    assert!(share.signature().is_valid());
    assert_eq!(share.address_id().as_str(), rikka_test_proton::ADDRESS_ID);
    let root = client.root(&share).await.unwrap();
    assert_eq!(root.name(), "root");
}

#[tokio::test(flavor = "multi_thread")]
async fn bookmarks_seal_password() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;

    client
        .save_bookmark(&format!("{PUBLIC_URL}#ABCDEFGHIJKL"), Some("secret"))
        .await
        .unwrap();
    let saved = &drive.last_body("POST /drive/v2/urls/token/bookmark")["BookmarkShareURL"];
    assert_eq!(saved["AddressKeyID"].as_str(), Some("address-key"));
    let sealed = drive
        .pgp()
        .decrypt()
        .with_key(drive.owner().address.clone())
        .verified_by(drive.owner().address.public_key())
        .message(saved["EncryptedUrlPassword"].as_str().unwrap())
        .unwrap();
    assert_eq!(sealed.data(), BOOKMARKED_PASSWORD.as_bytes());
    assert!(sealed.signature().is_valid());

    let listed = client.bookmarks().await.unwrap();
    assert_eq!(listed.len(), 2);
    let bookmark = listed[0].as_ref().unwrap();
    assert_eq!(bookmark.name(), BOOKMARKED_NAME);
    assert_eq!(bookmark.url(), format!("{PUBLIC_URL}#ABCDEFGHIJKL"));
    assert_eq!(
        bookmark.custom_password().map(|custom| custom.expose()),
        Some("secret")
    );
    assert!(listed[1].is_err(), "bookmark without password not reported");

    client.delete_bookmark(bookmark.token()).await.unwrap();
    assert_eq!(
        drive.request_count("DELETE /drive/v2/urls/token/bookmark"),
        1
    );
}
