use std::collections::HashSet;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use rikka_proton_api::offload;
use rikka_proton_drive::{
    DeviceId, DeviceSync, DeviceType, DriveClient, Error as DriveError, Folder, LinkId, Node,
    OpenedDevice, ShareKeys, VolumeId, VolumeType,
};
use rikka_proton_drive_sync_state::{Root, RootId, State, Store, Target};

use crate::engine::root::SyncRoot;
use crate::error::{Error, Result};
use crate::persist::Persist;
use crate::remote::tree::{RemoteRoot, open_folder};

const STATE_FILE: &str = "sync.sqlite";
const CLIENT_KEY: &str = "client";
const DEVICE_KEY: &str = "device";
const ENUMERATED_KEY: &str = "enumerated";

pub(crate) struct Setup {
    pub(crate) client: DriveClient,
    pub(crate) persist: Persist,
    pub(crate) roots: Vec<SyncRoot>,
    pub(crate) device: Option<DeviceId>,
}

#[derive(Clone)]
struct RootFolder {
    volume: VolumeId,
    share: ShareKeys,
    folder: Folder,
    is_new_device: bool,
}

pub(crate) fn enumerated_key(id: RootId) -> String {
    format!("{ENUMERATED_KEY}:{id}")
}

pub(crate) async fn set_up(
    client: DriveClient,
    state_folder: &Path,
    device_name: &str,
    folders: Vec<(PathBuf, Target)>,
) -> Result<Setup> {
    std::fs::create_dir_all(state_folder).map_err(Error::io(state_folder))?;
    let folders = validate_folders(folders)?;
    let path = state_folder.join(STATE_FILE);
    let store = offload(move || Store::open(path)).await?;
    let persist = Persist::new(store);
    let client = client.with_client_uid(client_uid(&persist).await?);
    let stored_roots = persist.run(|store| store.roots()).await?;

    let mut resolver = RootResolver {
        client: &client,
        persist: &persist,
        device_name,
        computer: None,
        my_files: None,
        device: None,
        claimed_folders: HashSet::new(),
    };
    let mut roots = Vec::with_capacity(folders.len());
    for (local_path, target) in folders {
        let base = resolver.base_folder(&target).await?;
        let stored_root = stored_roots
            .iter()
            .find(|(_, record)| record.local_path == local_path);
        let reusable = stored_root.filter(|(_, record)| {
            record.target == target
                && record.share == base.share.id().as_str()
                && !base.is_new_device
        });
        let synced = match reusable {
            Some((id, record)) => resolver.reuse_root(*id, record.clone(), base).await?,
            None => {
                if let Some((id, _)) = stored_root {
                    let id = *id;
                    persist.run(move |store| store.remove_root(id)).await?;
                }
                resolver.create_root(local_path, target, base).await?
            }
        };
        roots.push(synced);
    }
    let device = resolver.device;

    let root_ids: Vec<RootId> = roots.iter().map(|root| root.id).collect();
    for (id, _) in stored_roots {
        if !root_ids.contains(&id) {
            persist.run(move |store| store.remove_root(id)).await?;
        }
    }

    Ok(Setup {
        client,
        persist,
        roots,
        device,
    })
}

async fn client_uid(persist: &Persist) -> Result<String> {
    persist
        .run(|store| match store.value(CLIENT_KEY)? {
            Some(uid) => Ok(uid),
            None => {
                let uid = format!("rikka-sync-{}", rikka_proton_pgp::random_hex(16));
                store.set_value(CLIENT_KEY, &uid)?;
                Ok(uid)
            }
        })
        .await
}

struct RootResolver<'a> {
    client: &'a DriveClient,
    persist: &'a Persist,
    device_name: &'a str,
    computer: Option<RootFolder>,
    my_files: Option<RootFolder>,
    device: Option<DeviceId>,
    claimed_folders: HashSet<String>,
}

impl RootResolver<'_> {
    async fn base_folder(&mut self, target: &Target) -> Result<RootFolder> {
        match target {
            Target::Computer => {
                if let Some(computer) = &self.computer {
                    return Ok(computer.clone());
                }
                let (registered, is_new_device) =
                    register_device(self.client, self.persist, self.device_name).await?;
                self.device = Some(registered.device().id().clone());
                let (device, share, folder) = registered.into_parts();
                let base = RootFolder {
                    volume: device.volume_id().clone(),
                    share,
                    folder,
                    is_new_device,
                };
                self.computer = Some(base.clone());
                Ok(base)
            }
            Target::MyFiles(_) => {
                if let Some(my_files) = &self.my_files {
                    return Ok(my_files.clone());
                }
                let base = my_files_base(self.client).await?;
                self.my_files = Some(base.clone());
                Ok(base)
            }
        }
    }

    async fn reuse_root(&mut self, id: RootId, record: Root, base: RootFolder) -> Result<SyncRoot> {
        let remote = open_folder(
            self.client,
            &base.volume,
            &base.folder,
            &LinkId::from(record.remote_root.as_str()),
        )
        .await?
        .map(|folder| RemoteRoot::new(base.volume.clone(), base.share.clone(), folder));
        self.claimed_folders.insert(record.remote_root.clone());
        let key = enumerated_key(id);
        let loaded = record.clone();
        let (state, enumerated) = self
            .persist
            .run(move |store| Ok((store.load(id, &loaded)?, store.value(&key)?.is_some())))
            .await?;
        Ok(SyncRoot::new(id, record, state, remote, enumerated))
    }

    async fn create_root(
        &mut self,
        local_path: PathBuf,
        target: Target,
        base: RootFolder,
    ) -> Result<SyncRoot> {
        let folder = match &target {
            Target::Computer => {
                computer_root(self.client, &base, &local_path, &self.claimed_folders).await?
            }
            Target::MyFiles(path) => my_files_root(self.client, &base, path).await?,
        };
        self.claimed_folders.insert(folder.id().as_str().to_owned());
        let local_root = std::fs::symlink_metadata(&local_path)
            .map_err(Error::io(&local_path))?
            .ino();
        let record = Root {
            local_path,
            target,
            volume: base.volume.as_str().to_owned(),
            share: base.share.id().as_str().to_owned(),
            remote_root: folder.id().as_str().to_owned(),
            local_root,
        };
        let added = record.clone();
        let id = self
            .persist
            .run(move |store| store.add_root(&added))
            .await?;
        let state = State::new(record.local_root, record.remote_root.clone());
        let remote = Some(RemoteRoot::new(base.volume, base.share, folder));
        Ok(SyncRoot::new(id, record, state, remote, false))
    }
}

fn validate_folders(folders: Vec<(PathBuf, Target)>) -> Result<Vec<(PathBuf, Target)>> {
    if folders.is_empty() {
        return Err(Error::NoFolders);
    }
    let mut targets: Vec<(PathBuf, Target)> = Vec::with_capacity(folders.len());
    for (path, target) in folders {
        let path = path.canonicalize().map_err(Error::io(&path))?;
        if !path.is_dir() {
            return Err(Error::NotAFolder(path));
        }
        if path.file_name().and_then(|name| name.to_str()).is_none() {
            return Err(Error::InvalidName(path));
        }
        for (other, _) in &targets {
            if path.starts_with(other) || other.starts_with(&path) {
                let (outer, inner) = match path.starts_with(other) {
                    true => (other.clone(), path.clone()),
                    false => (path.clone(), other.clone()),
                };
                return Err(Error::NestedFolders { outer, inner });
            }
        }
        targets.push((path, target));
    }
    Ok(targets)
}

async fn register_device(
    client: &DriveClient,
    persist: &Persist,
    name: &str,
) -> Result<(OpenedDevice, bool)> {
    if let Some(id) = persist.run(|store| store.value(DEVICE_KEY)).await? {
        match client.device(&DeviceId::from(id)).await {
            Ok(device) => return Ok((client.open_device(device).await?, false)),
            Err(DriveError::UnknownDevice(_)) => {}
            Err(error) => return Err(error.into()),
        }
    }
    let registered_device = client.create_device(name, DeviceType::Linux).await?;
    client
        .set_device_sync(registered_device.device().id(), DeviceSync::On, None)
        .await?;
    let id = registered_device.device().id().as_str().to_owned();
    persist
        .run(move |store| store.set_value(DEVICE_KEY, &id))
        .await?;
    Ok((registered_device, true))
}

async fn my_files_base(client: &DriveClient) -> Result<RootFolder> {
    let volume = client
        .volumes()
        .await?
        .into_iter()
        .find(|volume| volume.is_active() && volume.kind() == VolumeType::Regular)
        .ok_or(Error::NoVolume)?;
    let share = client.open_share(volume.share().id()).await?;
    let folder = client.root(&share).await?;
    Ok(RootFolder {
        volume: volume.id().clone(),
        share,
        folder,
        is_new_device: false,
    })
}

async fn computer_root(
    client: &DriveClient,
    base: &RootFolder,
    local_path: &Path,
    claimed_folders: &HashSet<String>,
) -> Result<Folder> {
    let name = local_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::InvalidName(local_path.to_owned()))?;
    let children = client.children(&base.volume, &base.folder).await?;
    if let Some(folder) = children
        .into_iter()
        .filter_map(Node::into_folder)
        .find(|folder| folder.name() == name && !claimed_folders.contains(folder.id().as_str()))
    {
        return Ok(folder);
    }
    let available = client
        .available_folder_name(&base.volume, &base.folder, name)
        .await?;
    Ok(client
        .create_folder(&base.share, &base.folder, &available)
        .await?)
}

async fn my_files_root(client: &DriveClient, base: &RootFolder, path: &str) -> Result<Folder> {
    let mut folder = base.folder.clone();
    for segment in path.split('/').filter(|segment| !segment.is_empty()) {
        let children = client.children(&base.volume, &folder).await?;
        folder = match children
            .into_iter()
            .filter_map(Node::into_folder)
            .find(|child| child.name() == segment)
        {
            Some(child) => child,
            None => client.create_folder(&base.share, &folder, segment).await?,
        };
    }
    Ok(folder)
}
