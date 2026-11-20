use rikka_proton_drive::{DeviceId, DeviceSync, DeviceType};
use rikka_proton_pgp::LockedKey;
use rikka_test_proton_drive::{DEVICE, DummyDrive, ROOT, Shape};

#[tokio::test(flavor = "multi_thread")]
async fn new_device_gets_share_and_root() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;

    let device = client
        .create_device("Laptop", DeviceType::Linux)
        .await
        .unwrap();
    assert_eq!(device.device().kind(), DeviceType::Linux);
    assert_eq!(device.name(), "Laptop");
    let listed = client.devices().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id(), device.device().id());
    assert_eq!(listed[0].share_id(), device.share().id());

    let body = drive.last_body("POST /drive/devices");
    assert_eq!(body["Device"]["Type"].as_i64(), Some(3));
    assert_eq!(body["Device"]["SyncState"].as_i64(), Some(0));
    assert_eq!(body["Share"]["AddressKeyID"].as_str(), Some("address-key"));
    let share_passphrase = drive.decrypt_text(
        &drive.owner().address,
        body["Share"]["Passphrase"].as_str().unwrap(),
    );
    let share_key = LockedKey::import(body["Share"]["Key"].as_str().unwrap())
        .unwrap()
        .unlock(share_passphrase)
        .unwrap();
    assert_eq!(
        drive.decrypt_text(&share_key, body["Link"]["Name"].as_str().unwrap()),
        "Laptop"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn device_rename_updates_root() {
    let drive = DummyDrive::with_shape(Shape::new().legacy_device("Old laptop")).await;
    let client = drive.client().await;
    let listed = client.devices().await.unwrap();
    assert_eq!(listed[0].legacy_name(), Some("Old laptop"));
    let device = client.open_device(listed[0].clone()).await.unwrap();
    assert_eq!(device.name(), "Old laptop");

    client.rename_device(&device, "Desk").await.unwrap();

    assert_eq!(
        drive.last_body("PUT /drive/devices/device")["Share"]["Name"].as_str(),
        Some("")
    );
    let renamed = drive.last_body(&format!("PUT /drive/v2/volumes/volume/links/{ROOT}/rename"));
    assert!(renamed["Hash"].is_null(), "root has a name hash");
    assert_eq!(
        drive.decrypt_text(&drive.share().key, renamed["Name"].as_str().unwrap()),
        "Desk"
    );
    let reopened = client
        .open_device(client.devices().await.unwrap()[0].clone())
        .await
        .unwrap();
    assert_eq!(reopened.name(), "Desk");

    client
        .set_device_sync(device.device().id(), DeviceSync::On, None)
        .await
        .unwrap();
    assert_eq!(
        drive.last_body("PUT /drive/devices/device")["Device"]["SyncState"].as_i64(),
        Some(1)
    );

    client.delete_device(&DeviceId::from(DEVICE)).await.unwrap();
    assert_eq!(drive.request_count("DELETE /drive/devices/device"), 1);
    assert!(client.devices().await.unwrap().is_empty());

    let missing = client
        .device(&DeviceId::from("elsewhere"))
        .await
        .unwrap_err();
    assert!(
        matches!(missing, rikka_proton_drive::Error::UnknownDevice(_)),
        "{missing}"
    );
}
