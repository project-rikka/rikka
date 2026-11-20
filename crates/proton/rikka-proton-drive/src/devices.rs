use jiff::Timestamp;
use rikka_proton_api::offload;
use serde::Serialize;

use crate::bootstrap::{RootBootstrap, ShareBootstrap};
use crate::client::DriveClient;
use crate::device::{Device, DeviceSync, DeviceType, OpenedDevice};
use crate::error::{Error, Result};
use crate::identifier::DeviceId;
use crate::node::{Node, Parent};

impl DriveClient {
    pub async fn devices(&self) -> Result<Vec<Device>> {
        Ok(self
            .api()
            .get("drive/devices")
            .send_field_or_default("Devices")
            .await?)
    }

    pub async fn device(&self, id: &DeviceId) -> Result<Device> {
        self.devices()
            .await?
            .into_iter()
            .find(|device| device.id() == id)
            .ok_or_else(|| Error::UnknownDevice(id.clone()))
    }

    pub async fn create_device(&self, name: &str, kind: DeviceType) -> Result<OpenedDevice> {
        let client = self.clone();
        let named = name.to_owned();
        let bootstrap = offload(move || client.bootstrap(&named)).await?;
        let request = CreateDeviceRequest {
            device: NewDevice {
                kind,
                sync: DeviceSync::Off,
            },
            share: bootstrap.share,
            link: bootstrap.root,
        };

        let created: DeviceId = self
            .api()
            .post("drive/devices")
            .json(&request)
            .send_field_at(&["Device", "DeviceID"])
            .await?;

        let device = self.device(&created).await?;
        self.open_device(device).await
    }

    pub async fn open_device(&self, device: Device) -> Result<OpenedDevice> {
        let share = self.open_share(device.share_id()).await?;
        let root = self.root(&share).await?;

        Ok(OpenedDevice::new(device, share, root))
    }

    pub async fn rename_device(&self, device: &OpenedDevice, name: &str) -> Result<()> {
        if device.device().legacy_name().is_some() {
            self.update_device(
                device.device().id(),
                &UpdateDeviceRequest {
                    device: None,
                    share: Some(DeviceShareUpdate { name: "" }),
                },
            )
            .await?;
        }

        let root = Node::Folder(device.root().clone());
        self.rename(device.share(), Parent::Share(device.share()), &root, name)
            .await
    }

    pub async fn set_device_sync(
        &self,
        id: &DeviceId,
        sync: DeviceSync,
        last_synced_at: Option<Timestamp>,
    ) -> Result<()> {
        self.update_device(
            id,
            &UpdateDeviceRequest {
                device: Some(DeviceUpdate {
                    sync,
                    last_sync_time: last_synced_at,
                }),
                share: None,
            },
        )
        .await
    }

    pub async fn delete_device(&self, id: &DeviceId) -> Result<()> {
        self.api()
            .delete(format!("drive/devices/{id}"))
            .send_unit()
            .await?;

        Ok(())
    }

    async fn update_device(&self, id: &DeviceId, request: &UpdateDeviceRequest<'_>) -> Result<()> {
        self.api()
            .put(format!("drive/devices/{id}"))
            .json(request)
            .send_unit()
            .await?;

        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct CreateDeviceRequest {
    #[serde(rename = "Device")]
    device: NewDevice,
    #[serde(rename = "Share")]
    share: ShareBootstrap,
    #[serde(rename = "Link")]
    link: RootBootstrap,
}

#[derive(Debug, Serialize)]
struct NewDevice {
    #[serde(rename = "Type")]
    kind: DeviceType,
    #[serde(rename = "SyncState")]
    sync: DeviceSync,
}

#[derive(Debug, Serialize)]
struct UpdateDeviceRequest<'a> {
    #[serde(rename = "Device")]
    device: Option<DeviceUpdate>,
    #[serde(rename = "Share")]
    share: Option<DeviceShareUpdate<'a>>,
}

#[derive(Debug, Serialize)]
struct DeviceUpdate {
    #[serde(rename = "SyncState")]
    sync: DeviceSync,
    #[serde(
        rename = "LastSyncTime",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    last_sync_time: Option<Timestamp>,
}

#[derive(Debug, Serialize)]
struct DeviceShareUpdate<'a> {
    #[serde(rename = "Name")]
    name: &'a str,
}
