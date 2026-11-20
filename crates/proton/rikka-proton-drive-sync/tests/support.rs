use std::path::Path;

use rikka_proton_drive::{DeviceId, DriveClient, File, Folder, Node, ShareKeys, VolumeId};
use rikka_proton_drive_sync::DriveSync;
use rikka_test_proton_drive::DummyDrive;

pub(crate) const DEVICE: &str = "Laptop";

pub(crate) async fn computer(drive: &DummyDrive, folder: &Path, state: &Path) -> DriveSync {
    DriveSync::builder(drive.client().await)
        .state_folder(state)
        .device_name(DEVICE)
        .computer_folder(folder)
        .start()
        .await
        .unwrap()
}

pub(crate) async fn sync_now(sync: &DriveSync) {
    sync.sync_now().await.unwrap();
}

pub(crate) struct Remote {
    client: DriveClient,
    volume: VolumeId,
    share: ShareKeys,
    folder: Folder,
}

impl Remote {
    pub(crate) async fn device_folder(drive: &DummyDrive, device: &DeviceId, name: &str) -> Remote {
        let client = drive.client().await;
        let created_device = client
            .open_device(client.device(device).await.unwrap())
            .await
            .unwrap();
        let (device, share, root) = created_device.into_parts();
        let volume = device.volume_id().clone();
        let folder = client
            .children(&volume, &root)
            .await
            .unwrap()
            .into_iter()
            .filter_map(Node::into_folder)
            .find(|folder| folder.name() == name)
            .unwrap();
        Remote {
            client,
            volume,
            share,
            folder,
        }
    }

    pub(crate) async fn my_files(drive: &DummyDrive, path: &[&str]) -> Remote {
        let client = drive.client().await;
        let volume = client.volumes().await.unwrap().into_iter().next().unwrap();
        let share = client.open_share(volume.share().id()).await.unwrap();
        let mut folder = client.root(&share).await.unwrap();
        for name in path {
            folder = client
                .children(volume.id(), &folder)
                .await
                .unwrap()
                .into_iter()
                .filter_map(Node::into_folder)
                .find(|child| child.name() == *name)
                .unwrap();
        }
        Remote {
            client,
            volume: volume.id().clone(),
            share,
            folder,
        }
    }

    pub(crate) fn client(&self) -> &DriveClient {
        &self.client
    }

    pub(crate) fn volume(&self) -> &VolumeId {
        &self.volume
    }

    pub(crate) async fn children(&self) -> Vec<Node> {
        self.client
            .children(&self.volume, &self.folder)
            .await
            .unwrap()
    }

    pub(crate) async fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .children()
            .await
            .iter()
            .map(|node| node.name().to_owned())
            .collect();
        names.sort();
        names
    }

    pub(crate) async fn file(&self, name: &str) -> File {
        self.children()
            .await
            .into_iter()
            .filter_map(Node::into_file)
            .find(|file| file.name() == name)
            .unwrap_or_else(|| panic!("{name} missing from remote folder"))
    }

    pub(crate) async fn read(&self, name: &str) -> Vec<u8> {
        let file = self.file(name).await;
        self.client
            .download(&self.volume, &file)
            .into_vec()
            .await
            .unwrap()
    }

    pub(crate) async fn upload(&self, name: &str, content: &[u8]) {
        self.client
            .upload(&self.share, &self.folder, name)
            .upload_bytes(content.to_vec())
            .await
            .unwrap();
    }

    pub(crate) async fn revise(&self, name: &str, content: &[u8]) {
        let file = self.file(name).await;
        self.client
            .upload_revision(&self.share, &self.folder, &file)
            .upload_bytes(content.to_vec())
            .await
            .unwrap();
    }

    pub(crate) async fn trash(&self, name: &str) {
        let file = self.file(name).await;
        self.client
            .trash(&self.volume, std::slice::from_ref(file.id()))
            .await
            .unwrap();
    }

    pub(crate) async fn folder(&self, name: &str) -> Remote {
        let folder = self
            .children()
            .await
            .into_iter()
            .filter_map(Node::into_folder)
            .find(|folder| folder.name() == name)
            .unwrap_or_else(|| panic!("{name} is not a remote folder"));
        Remote {
            client: self.client.clone(),
            volume: self.volume.clone(),
            share: self.share.clone(),
            folder,
        }
    }
}

pub(crate) fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with(".~") && !name.starts_with(".Trash"))
        .collect();
    names.sort();
    names
}
