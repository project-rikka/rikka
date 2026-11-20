use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::{Value, json};

use rikka_test_proton::{
    ADDRESS_ID, EMAIL, Request, ResponseFuture, delayed, json_response, not_found, response,
    success,
};

use crate::digest::{base64_digest, lookup_hash, manifest, sha1_hex};
use crate::fixture::{Fixture, NOW};
use crate::ids::{
    DEVICE, DRAFT_NAME, ELSEWHERE, FILE, FILE_NAME, FOLDER, FOLDER_NAME, KNOWN_THUMBNAIL,
    OLDER_REVISION, REVISION, ROOT, SHARE, SHOT, SHOT_BURST, SHOT_MOTION, STALE, STALE_DRAFT,
    THUMBNAIL, UNFINISHED_REVISION, VOLUME, extra_folder_id,
};
use crate::shape::{Conflict, Corruption, Shape};

const CHILDREN_PAGE: usize = 150;
const SLOW_STEPS: usize = 7;
const SLOW_STEP: Duration = Duration::from_millis(15);

pub(crate) const DRAFT: i64 = 0;
pub(crate) const ACTIVE: i64 = 1;
const TRASHED: i64 = 2;
const OBSOLETE: i64 = 2;

pub(crate) const FOLDER_KIND: i64 = 1;
pub(crate) const FILE_KIND: i64 = 2;
pub(crate) const ALBUM_KIND: i64 = 3;

const DELETE: i64 = 0;
const CREATE: i64 = 1;
const UPDATE: i64 = 2;
const UPDATE_METADATA: i64 = 3;

const MAIN_SHARE_KIND: i64 = 1;
const DEVICE_SHARE_KIND: i64 = 3;

struct StoredShare {
    id: String,
    kind: i64,
    root: String,
    key: String,
    passphrase: String,
    passphrase_signature: String,
}

struct Device {
    id: String,
    share: String,
    root: String,
    kind: i64,
    sync: i64,
    synced_at: Option<i64>,
    name: String,
}

pub(crate) struct Link {
    pub(crate) id: String,
    volume: &'static str,
    pub(crate) parent: Option<String>,
    pub(crate) kind: i64,
    pub(crate) state: i64,
    pub(crate) name: String,
    pub(crate) hash: Option<String>,
    pub(crate) node_key: String,
    pub(crate) node_passphrase: String,
    pub(crate) node_passphrase_signature: String,
    pub(crate) hash_key: Option<String>,
    pub(crate) content_key_packet: Option<String>,
    pub(crate) content_key_packet_signature: Option<String>,
    pub(crate) media_type: Option<String>,
    client_uid: Option<String>,
    pub(crate) revisions: Vec<Revision>,
    modified: i64,
    order: u64,
    shared: bool,
}

pub(crate) struct Revision {
    pub(crate) id: String,
    pub(crate) state: i64,
    client_uid: Option<String>,
    blocks: BTreeMap<u32, Block>,
    thumbnails: Vec<Thumbnail>,
    pub(crate) manifest_signature: Option<String>,
    attributes: Option<String>,
    photo: Option<Value>,
    checksum_verified: bool,
}

struct Block {
    hash: String,
    signature: String,
    token: String,
}

struct Thumbnail {
    kind: i64,
    token: String,
}

enum Upload {
    Block {
        link: String,
        revision: String,
        index: u32,
        hash: String,
        signature: String,
    },
    Thumbnail {
        link: String,
        revision: String,
        kind: i64,
    },
}

struct Recorded {
    route: String,
    body: Value,
}

pub(crate) struct DriveState {
    pub(crate) fixture: Arc<Fixture>,
    pub(crate) shape: Shape,
    base: String,
    issued: u64,
    shares: Vec<StoredShare>,
    devices: Vec<Device>,
    pub(crate) links: HashMap<String, Link>,
    events: Vec<Value>,
    pending: HashMap<String, Upload>,
    blobs: HashMap<String, Vec<u8>>,
    uploaded: HashMap<u32, Vec<u8>>,
    recorded: Vec<Recorded>,
    block_uploads: usize,
    stall_after: Option<usize>,
    reading: Arc<AtomicUsize>,
    max_reading: Arc<AtomicUsize>,
    reads: usize,
}

impl DriveState {
    pub(crate) fn new(fixture: Arc<Fixture>, shape: Shape) -> DriveState {
        let mut state = DriveState {
            shares: vec![StoredShare {
                id: SHARE.to_owned(),
                kind: MAIN_SHARE_KIND,
                root: ROOT.to_owned(),
                key: fixture.share.armored_key.clone(),
                passphrase: fixture.share.armored_passphrase.clone(),
                passphrase_signature: fixture.share.armored_signature.clone(),
            }],
            fixture,
            shape,
            base: String::new(),
            issued: 0,
            devices: Vec::new(),
            links: HashMap::new(),
            events: Vec::new(),
            pending: HashMap::new(),
            blobs: HashMap::new(),
            uploaded: HashMap::new(),
            recorded: Vec::new(),
            block_uploads: 0,
            stall_after: None,
            reading: Arc::new(AtomicUsize::new(0)),
            max_reading: Arc::new(AtomicUsize::new(0)),
            reads: 0,
        };
        state.seed();
        state
    }

    fn seed(&mut self) {
        let fixture = Arc::clone(&self.fixture);
        let root_name = fixture.owner.name(&fixture.share.key.public_key(), "root");
        self.insert(Link::folder(
            ROOT,
            None,
            root_name,
            None,
            &fixture.root,
            &fixture.root_hash.armored,
        ));
        if let Some(name) = self.shape.legacy_device.clone() {
            self.devices.push(Device {
                id: DEVICE.to_owned(),
                share: SHARE.to_owned(),
                root: ROOT.to_owned(),
                kind: DEVICE_SHARE_KIND,
                sync: 0,
                synced_at: None,
                name,
            });
        }
        if !self.shape.seeded {
            return;
        }

        let mut folder = Link::folder(
            FOLDER,
            Some(ROOT),
            fixture.root_name(FOLDER_NAME),
            Some(lookup_hash(&fixture.root_hash.secret, FOLDER_NAME)),
            &fixture.folder,
            &fixture.folder_hash.armored,
        );
        if self.shape.corruption == Corruption::Passphrase {
            folder.node_passphrase_signature = fixture.address_signature("another passphrase");
        }
        self.insert(folder);

        let revision = self.seeded_revision(REVISION);
        self.insert(self.seeded_file(FILE, ROOT, VOLUME, FILE_NAME, vec![revision]));
        for name in self.shape.extra_files.clone() {
            let id = self.issue("file");
            let revision_id = self.issue("revision");
            let revision = self.seeded_revision(&revision_id);
            self.insert(self.seeded_file(&id, ROOT, VOLUME, &name, vec![revision]));
        }
        for at in 0..self.shape.extra_folders {
            let id = extra_folder_id(at);
            self.insert(Link::folder(
                &id,
                Some(ROOT),
                fixture.root_name(&id),
                Some(lookup_hash(&fixture.root_hash.secret, &id)),
                &fixture.folder,
                &fixture.folder_hash.armored,
            ));
        }
        if self.shape.revision_history {
            self.seed_history();
        }
        if let Some(conflict) = self.shape.conflict {
            self.seed_stale_draft(conflict);
        }
        self.seed_elsewhere();
    }

    fn seeded_revision(&mut self, id: &str) -> Revision {
        let fixture = Arc::clone(&self.fixture);
        let thumbnail = fixture.sealed_thumbnail(THUMBNAIL);
        let mut sealed = Vec::new();
        let mut blocks = BTreeMap::new();
        for (at, plain) in self.shape.blocks.iter().enumerate() {
            let (data, signature) = fixture.sealed_block(plain);
            let token = format!("{id}-block-{at}");
            blocks.insert(
                u32::try_from(at + 1).unwrap(),
                Block {
                    hash: base64_digest(&data),
                    signature,
                    token: token.clone(),
                },
            );
            sealed.push(data);
        }
        let covered: Vec<Vec<u8>> = std::iter::once(thumbnail.clone())
            .chain(sealed.iter().cloned())
            .collect();
        let manifest_signature = fixture.address_signature(match self.shape.corruption {
            Corruption::Manifest => b"another manifest".to_vec(),
            _ => manifest(&covered),
        });
        if self.shape.corruption == Corruption::Block
            && let Some(first) = sealed.first_mut()
        {
            *first = b"not the block the hash names".to_vec();
        }
        for (block, data) in blocks.values().zip(sealed) {
            self.blobs.insert(block.token.clone(), data);
        }
        let thumbnail_token = match id {
            REVISION => KNOWN_THUMBNAIL.to_owned(),
            _ => format!("{id}-thumbnail"),
        };
        self.blobs.insert(thumbnail_token.clone(), thumbnail);

        let sizes: Vec<usize> = self.shape.blocks.iter().map(Vec::len).collect();
        let attributes = fixture.attributes(
            &json!({
                "Common": {
                    "Size": sizes.iter().sum::<usize>(),
                    "BlockSizes": sizes,
                    "Digests": {"SHA1": sha1_hex(&self.shape.blocks.concat())},
                },
            })
            .to_string(),
        );

        Revision {
            id: id.to_owned(),
            state: ACTIVE,
            client_uid: None,
            blocks,
            thumbnails: vec![Thumbnail {
                kind: 1,
                token: thumbnail_token,
            }],
            manifest_signature: Some(manifest_signature),
            attributes: Some(attributes),
            photo: None,
            checksum_verified: true,
        }
    }

    fn seeded_file(
        &self,
        id: &str,
        parent: &str,
        volume: &'static str,
        name: &str,
        revisions: Vec<Revision>,
    ) -> Link {
        let fixture = &self.fixture;
        Link {
            id: id.to_owned(),
            volume,
            parent: Some(parent.to_owned()),
            kind: FILE_KIND,
            state: ACTIVE,
            name: fixture.root_name(name),
            hash: Some(lookup_hash(&fixture.root_hash.secret, name)),
            node_key: fixture.file.armored_key.clone(),
            node_passphrase: fixture.file.armored_passphrase.clone(),
            node_passphrase_signature: fixture.file.armored_signature.clone(),
            hash_key: None,
            content_key_packet: Some(fixture.content_packet.clone()),
            content_key_packet_signature: Some(fixture.content_signature.clone()),
            media_type: Some("text/plain".to_owned()),
            client_uid: None,
            revisions,
            modified: NOW,
            order: 0,
            shared: false,
        }
    }

    fn seed_history(&mut self) {
        let (data, _) = self.fixture.sealed_block(b"old");
        self.blobs.insert("older-block".to_owned(), data.clone());
        let older = Revision {
            id: OLDER_REVISION.to_owned(),
            state: OBSOLETE,
            client_uid: None,
            blocks: BTreeMap::from([(
                1,
                Block {
                    hash: base64_digest(&data),
                    signature: String::new(),
                    token: "older-block".to_owned(),
                },
            )]),
            thumbnails: Vec::new(),
            manifest_signature: None,
            attributes: None,
            photo: None,
            checksum_verified: false,
        };
        let file = self.links.get_mut(FILE).unwrap();
        file.revisions.push(older);
        file.revisions
            .push(Revision::draft(UNFINISHED_REVISION.to_owned(), None));
    }

    fn seed_stale_draft(&mut self, conflict: Conflict) {
        let client = match conflict {
            Conflict::OwnDraft => "rikka-sync",
            Conflict::ForeignDraft => "another-client",
        };
        let mut stale = self.seeded_file(STALE, ROOT, VOLUME, DRAFT_NAME, Vec::new());
        stale.state = DRAFT;
        stale.client_uid = Some(client.to_owned());
        stale.revisions = vec![Revision::draft(
            STALE_DRAFT.to_owned(),
            Some(client.to_owned()),
        )];
        self.insert(stale);
    }

    fn seed_elsewhere(&mut self) {
        for (id, name, main, related) in [
            (SHOT, "shot.jpg", None, vec![SHOT_MOTION]),
            (SHOT_MOTION, "shot.mov", Some(SHOT), Vec::new()),
            (SHOT_BURST, "shot-2.jpg", Some(SHOT), Vec::new()),
        ] {
            let mut revision = self.seeded_revision(&format!("{id}-revision"));
            revision.photo = Some(json!({
                "CaptureTime": 1_650_000_000,
                "MainPhotoLinkID": main,
                "ContentHash": "foreign-content",
                "RelatedPhotosLinkIDs": related,
            }));
            let link = self.seeded_file(id, ROOT, ELSEWHERE, name, vec![revision]);
            self.insert(link);
        }
    }

    pub(crate) fn insert(&mut self, mut link: Link) {
        self.issued += 1;
        link.order = self.issued;
        self.links.insert(link.id.clone(), link);
    }

    pub(crate) fn set_base(&mut self, base: String) {
        self.base = base;
    }

    pub(crate) fn record(&mut self, request: &Request) {
        self.recorded.push(Recorded {
            route: format!("{} {}", request.method, request.path),
            body: request.json(),
        });
    }

    pub(crate) fn bodies(&self, pattern: &str) -> Vec<Value> {
        self.recorded
            .iter()
            .filter(|recorded| matches_route(pattern, &recorded.route))
            .map(|recorded| recorded.body.clone())
            .collect()
    }

    pub(crate) fn request_count(&self, pattern: &str) -> usize {
        self.recorded
            .iter()
            .filter(|recorded| matches_route(pattern, &recorded.route))
            .count()
    }

    pub(crate) fn uploaded(&self, index: u32) -> Vec<u8> {
        self.uploaded.get(&index).cloned().unwrap_or_default()
    }

    pub(crate) fn max_concurrent_reads(&self) -> usize {
        self.max_reading.load(Ordering::SeqCst)
    }

    pub(crate) fn stall_uploads_after(&mut self, count: Option<usize>) {
        self.stall_after = count;
    }

    pub(crate) fn record_block_upload(&mut self, request: &Request) -> Option<usize> {
        if request.method != "POST" || !request.path.starts_with("/storage/") {
            return None;
        }
        self.block_uploads += 1;
        Some(self.block_uploads)
    }

    pub(crate) fn is_stalled(&self, upload: usize) -> bool {
        self.stall_after.is_some_and(|count| upload > count)
    }

    pub(crate) fn respond_core(&mut self, request: &Request) -> ResponseFuture {
        let segments: Vec<&str> = request.path.trim_start_matches('/').split('/').collect();
        let method = request.method.as_str();
        match segments.as_slice() {
            ["drive", "v2", "volumes", volume, rest @ ..] => {
                self.respond_volume(method, volume, rest, request)
            }
            ["drive", "shares", rest @ ..] => self.respond_shares(method, rest),
            ["drive", "devices", rest @ ..] => self.respond_devices(method, rest, request),
            ["storage", token] => self.respond_storage(method, token, request),
            ["drive", "volumes"] if method == "GET" => {
                json_response(200, &json!({"Code": 1000, "Volumes": [volume()]}))
            }
            ["drive", "volumes", VOLUME] if method == "GET" => {
                json_response(200, &json!({"Code": 1000, "Volume": volume()}))
            }
            ["drive", "volumes", VOLUME, "events", "latest"] if method == "GET" => json_response(
                200,
                &json!({"Code": 1000, "EventID": event_id(self.events.len())}),
            ),
            ["drive", "volumes", VOLUME, "thumbnails"] if method == "POST" => {
                self.thumbnail_addresses(&request.json())
            }
            ["drive", "blocks"] if method == "POST" => self.blocks(&request.json()),
            _ => not_found(),
        }
    }

    fn respond_shares(&self, method: &str, rest: &[&str]) -> ResponseFuture {
        match (method, rest) {
            ("GET", []) => {
                let shares: Vec<Value> = self
                    .shares
                    .iter()
                    .map(|share| self.share_json(share))
                    .collect();
                json_response(200, &json!({"Code": 1000, "Shares": shares}))
            }
            ("GET", [id]) => match self.shares.iter().find(|candidate| candidate.id == *id) {
                Some(share) => json_response(200, &self.share_json(share)),
                None => not_found(),
            },
            _ => not_found(),
        }
    }

    fn respond_devices(
        &mut self,
        method: &str,
        rest: &[&str],
        request: &Request,
    ) -> ResponseFuture {
        match (method, rest) {
            ("GET", []) => {
                let devices: Vec<Value> = self.devices.iter().map(device_json).collect();
                json_response(200, &json!({"Code": 1000, "Devices": devices}))
            }
            ("POST", []) => self.create_device(&request.json()),
            ("PUT", [id]) => self.update_device(id, &request.json()),
            ("DELETE", [id]) => {
                self.devices.retain(|candidate| candidate.id != *id);
                success()
            }
            _ => not_found(),
        }
    }

    fn respond_storage(&mut self, method: &str, token: &str, request: &Request) -> ResponseFuture {
        match method {
            "POST" if self.shape.failing_upload => response(500, "{}"),
            "POST" => self.store_block(token, request),
            "GET" => match self.blobs.get(token) {
                Some(data) if self.shape.slow_blocks => self.slow_read(data.clone()),
                Some(data) => response(200, data.clone()),
                None => not_found(),
            },
            _ => not_found(),
        }
    }

    fn slow_read(&mut self, data: Vec<u8>) -> ResponseFuture {
        self.reads += 1;
        let reading = Arc::clone(&self.reading);
        let now = reading.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_reading.fetch_max(now, Ordering::SeqCst);
        let steps = u32::try_from(SLOW_STEPS - self.reads % SLOW_STEPS).unwrap_or(1);
        let read = delayed(SLOW_STEP * steps, response(200, data));
        Box::pin(async move {
            let response = read.await;
            reading.fetch_sub(1, Ordering::SeqCst);
            response
        })
    }

    fn respond_volume(
        &mut self,
        method: &str,
        volume: &str,
        rest: &[&str],
        request: &Request,
    ) -> ResponseFuture {
        if method == "POST" && rest == ["links"] {
            let links: Vec<Value> = ids(&request.json())
                .iter()
                .filter_map(|id| self.links.get(id.as_str()))
                .filter(|link| link.volume == volume)
                .map(|link| self.link_json(link))
                .collect();
            return json_response(200, &json!({"Code": 1000, "Links": links}));
        }
        if volume != VOLUME {
            return not_found();
        }
        match (method, rest) {
            ("GET", ["events", since]) => self.events_since(since),
            ("GET", ["folders", folder, "children"]) => {
                self.children(folder, request.query("AnchorID"))
            }
            ("POST", ["folders"]) => self.create_folder(&request.json()),
            ("POST", ["files"]) => self.create_file(&request.json()),
            ("POST", ["documents"]) => self.create_document(&request.json()),
            ("POST", ["files", link, "revisions"]) => self.create_revision(link, &request.json()),
            ("GET", ["files", link, "revisions"]) => self.revisions(link),
            ("GET", ["files", link, "revisions", revision]) => {
                self.revision(link, revision, request)
            }
            ("PUT", ["files", link, "revisions", revision]) => {
                self.commit(link, revision, &request.json())
            }
            ("POST", ["files", link, "revisions", revision, "restore"]) => {
                self.restore_revision(link, revision)
            }
            ("DELETE", ["files", link, "revisions", revision]) => {
                self.delete_revision(link, revision)
            }
            ("GET", ["links", link, "revisions", revision, "verification"]) => {
                self.verification(link, revision)
            }
            ("PUT", ["links", link, "rename"]) => self.rename(link, &request.json()),
            ("PUT", ["links", link, "move"]) => self.move_link(link, &request.json()),
            ("POST", ["links", folder, "checkAvailableHashes"]) => {
                self.available(folder, &request.json())
            }
            ("POST", ["trash_multiple"]) => self.set_state(&request.json(), TRASHED),
            ("PUT", ["trash", "restore_multiple"]) => self.set_state(&request.json(), ACTIVE),
            ("POST", ["trash", "delete_multiple"]) => self.remove(&request.json(), false),
            ("POST", ["delete_multiple"]) => {
                let attempts = self.request_count("POST /drive/v2/volumes/volume/delete_multiple");
                if self.shape.flaky_delete && attempts == 1 {
                    return refused(2000, Value::Null);
                }
                self.remove(&request.json(), true)
            }
            _ => not_found(),
        }
    }

    fn update_device(&mut self, id: &str, request: &Value) -> ResponseFuture {
        if let Some(device) = self.devices.iter_mut().find(|candidate| candidate.id == id) {
            if request["Device"].is_object() {
                if let Some(sync) = request["Device"]["SyncState"].as_i64() {
                    device.sync = sync;
                }
                device.synced_at = request["Device"]["LastSyncTime"].as_i64();
            }
            if let Some(name) = request["Share"]["Name"].as_str() {
                device.name = name.to_owned();
            }
        }
        success()
    }

    pub(crate) fn issue(&mut self, prefix: &str) -> String {
        self.issued += 1;
        format!("{prefix}-{}", self.issued)
    }

    fn event(&mut self, action: i64, link: &str) {
        let (parent, trashed, shared) = match self.links.get(link) {
            Some(candidate) => (
                candidate.parent.clone(),
                candidate.state == TRASHED,
                candidate.shared,
            ),
            None => (None, false, false),
        };
        let id = event_id(self.events.len() + 1);
        self.events.push(json!({
            "EventID": id,
            "EventType": action,
            "Link": {"LinkID": link, "ParentLinkID": parent, "IsTrashed": trashed, "IsShared": shared},
        }));
    }

    pub(crate) fn mark_shared(&mut self, link: &str) {
        if let Some(candidate) = self.links.get_mut(link) {
            candidate.shared = true;
        }
        self.event(UPDATE_METADATA, link);
    }

    fn events_since(&self, since: &str) -> ResponseFuture {
        let Some(start) = since
            .strip_prefix("event-")
            .and_then(|number| number.parse::<usize>().ok())
        else {
            return json_response(
                200,
                &json!({
                    "Code": 1000, "EventID": event_id(self.events.len()), "Events": [], "More": 0, "Refresh": 1,
                }),
            );
        };
        let start = start.min(self.events.len());
        let end = (start + self.shape.events_page).min(self.events.len());
        let convertible: Vec<Value> = match end == self.events.len() {
            true => self
                .shape
                .convertible
                .iter()
                .map(|link| json!({"LinkID": link}))
                .collect(),
            false => Vec::new(),
        };
        json_response(
            200,
            &json!({
                "Code": 1000,
                "EventID": event_id(end),
                "Events": self.events[start..end],
                "More": i64::from(end < self.events.len()),
                "Refresh": 0,
                "ConvertibleExternalInvitations": convertible,
            }),
        )
    }

    fn create_device(&mut self, request: &Value) -> ResponseFuture {
        let share_id = self.issue("share");
        let root_id = self.issue("link");
        let device_id = self.issue("device");
        self.shares.push(StoredShare {
            id: share_id.clone(),
            kind: DEVICE_SHARE_KIND,
            root: root_id.clone(),
            key: text(&request["Share"]["Key"]),
            passphrase: text(&request["Share"]["Passphrase"]),
            passphrase_signature: text(&request["Share"]["PassphraseSignature"]),
        });
        let link = &request["Link"];
        self.insert(
            Link::sent(&root_id, None, FOLDER_KIND, link).with_hash_key(text(&link["NodeHashKey"])),
        );
        self.devices.push(Device {
            id: device_id.clone(),
            share: share_id.clone(),
            root: root_id.clone(),
            kind: request["Device"]["Type"]
                .as_i64()
                .unwrap_or(DEVICE_SHARE_KIND),
            sync: request["Device"]["SyncState"].as_i64().unwrap_or_default(),
            synced_at: None,
            name: String::new(),
        });
        json_response(
            200,
            &json!({
                "Code": 1000,
                "Device": {"DeviceID": device_id, "ShareID": share_id, "LinkID": root_id},
            }),
        )
    }

    pub(crate) fn occupant(&self, parent: &str, hash: &str, except: Option<&str>) -> Option<&Link> {
        self.links.values().find(|candidate| {
            candidate.volume == VOLUME
                && candidate.parent.as_deref() == Some(parent)
                && candidate.hash.as_deref() == Some(hash)
                && matches!(candidate.state, DRAFT | ACTIVE)
                && Some(candidate.id.as_str()) != except
        })
    }

    fn create_folder(&mut self, request: &Value) -> ResponseFuture {
        let parent = text(&request["ParentLinkID"]);
        let hash = text(&request["Hash"]);
        if let Some(candidate) = self.occupant(&parent, &hash, None) {
            return clash(&candidate.id, None);
        }
        let id = self.issue("link");
        self.insert(
            Link::sent(&id, Some(parent), FOLDER_KIND, request)
                .with_hash_key(text(&request["NodeHashKey"])),
        );
        self.event(CREATE, &id);
        json_response(200, &json!({"Code": 1000, "Folder": {"ID": id}}))
    }

    fn create_file(&mut self, request: &Value) -> ResponseFuture {
        let parent = text(&request["ParentLinkID"]);
        let hash = text(&request["Hash"]);
        if let Some(candidate) = self.occupant(&parent, &hash, None) {
            return match candidate.state {
                DRAFT => clash(
                    &candidate.id,
                    candidate
                        .revisions
                        .iter()
                        .find(|revision| revision.state == DRAFT)
                        .map(|revision| (revision.id.as_str(), candidate.client_uid.as_deref())),
                ),
                _ => clash(&candidate.id, None),
            };
        }
        let id = self.issue("link");
        let revision = self.issue("revision");
        let client_uid = request["ClientUID"].as_str().map(str::to_owned);
        self.insert(Link {
            state: DRAFT,
            client_uid: client_uid.clone(),
            revisions: vec![Revision::draft(revision.clone(), client_uid)],
            ..Link::sent_file(&id, parent, request)
        });
        json_response(
            200,
            &json!({
                "Code": 1000,
                "File": {"ID": id, "RevisionID": revision, "ClientUID": request["ClientUID"]},
            }),
        )
    }

    fn create_document(&mut self, request: &Value) -> ResponseFuture {
        let parent = text(&request["ParentLinkID"]);
        let id = crate::ids::DOCUMENT.to_owned();
        let mut revision = Revision::draft("page".to_owned(), None);
        revision.state = ACTIVE;
        revision.manifest_signature = request["ManifestSignature"].as_str().map(str::to_owned);
        self.insert(Link {
            media_type: Some("application/vnd.proton.doc".to_owned()),
            revisions: vec![revision],
            ..Link::sent_file(&id, parent, request)
        });
        self.event(CREATE, &id);
        json_response(
            200,
            &json!({
                "Code": 1000,
                "Document": {"VolumeID": VOLUME, "LinkID": id, "RevisionID": "page"},
            }),
        )
    }

    fn create_revision(&mut self, link: &str, request: &Value) -> ResponseFuture {
        let revision = self.issue("revision");
        let Some(candidate) = self.links.get_mut(link) else {
            return not_found();
        };
        if let Some(draft) = candidate
            .revisions
            .iter()
            .find(|revision_entry| revision_entry.state == DRAFT)
        {
            return clash(
                &candidate.id,
                Some((draft.id.as_str(), draft.client_uid.as_deref())),
            );
        }
        let client_uid = request["ClientUID"].as_str().map(str::to_owned);
        candidate
            .revisions
            .push(Revision::draft(revision.clone(), client_uid));
        json_response(200, &json!({"Code": 1000, "Revision": {"ID": revision}}))
    }

    fn verification(&self, link: &str, revision: &str) -> ResponseFuture {
        let Some(candidate) = self.links.get(link) else {
            return not_found();
        };
        if !candidate
            .revisions
            .iter()
            .any(|revision_entry| revision_entry.id == revision)
        {
            return not_found();
        }
        json_response(
            200,
            &json!({
                "Code": 1000,
                "VerificationCode": rikka_proton_api::base64::encode(self.fixture.verification),
                "ContentKeyPacket": candidate.content_key_packet,
            }),
        )
    }

    fn blocks(&mut self, request: &Value) -> ResponseFuture {
        let link = text(&request["LinkID"]);
        let revision = text(&request["RevisionID"]);
        let mut links = Vec::new();
        for block in list(&request["BlockList"]) {
            let token = self.issue("token");
            let index = u32::try_from(block["Index"].as_u64().unwrap_or_default()).unwrap_or(0);
            self.pending.insert(
                token.clone(),
                Upload::Block {
                    link: link.clone(),
                    revision: revision.clone(),
                    index,
                    hash: text(&block["Hash"]),
                    signature: text(&block["EncSignature"]),
                },
            );
            links.push(json!({
                "Index": index,
                "Token": token,
                "BareURL": self.storage(&token),
                "URL": self.storage(&token),
            }));
        }
        let mut thumbnails = Vec::new();
        for thumbnail in list(&request["ThumbnailList"]) {
            let token = self.issue("token");
            let kind = thumbnail["Type"].as_i64().unwrap_or(1);
            self.pending.insert(
                token.clone(),
                Upload::Thumbnail {
                    link: link.clone(),
                    revision: revision.clone(),
                    kind,
                },
            );
            thumbnails.push(json!({
                "ThumbnailType": kind,
                "Token": token,
                "BareURL": self.storage(&token),
                "URL": self.storage(&token),
            }));
        }
        json_response(
            200,
            &json!({"Code": 1000, "UploadLinks": links, "ThumbnailLinks": thumbnails}),
        )
    }

    fn storage(&self, token: &str) -> String {
        format!("{}/storage/{token}", self.base)
    }

    fn store_block(&mut self, token: &str, request: &Request) -> ResponseFuture {
        let Some(data) = request.field("Block") else {
            return json_response(400, &json!({"Code": 2000, "Error": "no block"}));
        };
        self.blobs.insert(token.to_owned(), data.clone());
        match self.pending.remove(token) {
            Some(Upload::Block {
                link,
                revision,
                index,
                hash,
                signature,
            }) => {
                self.uploaded.insert(index, data);
                if let Some(revision) = self.revision_mut(&link, &revision) {
                    revision.blocks.insert(
                        index,
                        Block {
                            hash,
                            signature,
                            token: token.to_owned(),
                        },
                    );
                }
            }
            Some(Upload::Thumbnail {
                link,
                revision,
                kind,
            }) => {
                if kind == 1 {
                    self.uploaded.insert(0, data);
                }
                if let Some(revision) = self.revision_mut(&link, &revision) {
                    revision.thumbnails.push(Thumbnail {
                        kind,
                        token: token.to_owned(),
                    });
                }
            }
            None => {}
        }
        success()
    }

    fn revision_mut(&mut self, link: &str, revision: &str) -> Option<&mut Revision> {
        self.links
            .get_mut(link)?
            .revisions
            .iter_mut()
            .find(|entry| entry.id == revision)
    }

    fn commit(&mut self, link: &str, revision: &str, request: &Value) -> ResponseFuture {
        let Some(candidate) = self.links.get_mut(link) else {
            return not_found();
        };
        if !candidate
            .revisions
            .iter()
            .any(|revision_entry| revision_entry.id == revision)
        {
            return not_found();
        }
        for revision_entry in &mut candidate.revisions {
            if revision_entry.id == revision {
                revision_entry.state = ACTIVE;
                revision_entry.manifest_signature =
                    request["ManifestSignature"].as_str().map(str::to_owned);
                revision_entry.attributes = request["XAttr"].as_str().map(str::to_owned);
                revision_entry.photo = request.get("Photo").cloned();
                revision_entry.checksum_verified =
                    request["ChecksumVerified"].as_bool().unwrap_or(false);
            } else if revision_entry.state == ACTIVE {
                revision_entry.state = OBSOLETE;
            }
        }
        let created = candidate.state == DRAFT;
        candidate.state = ACTIVE;
        candidate.modified += 1;
        self.event(if created { CREATE } else { UPDATE }, link);
        success()
    }

    fn restore_revision(&mut self, link: &str, revision: &str) -> ResponseFuture {
        let Some(candidate) = self.links.get_mut(link) else {
            return not_found();
        };
        if !candidate.revisions.iter().any(|entry| entry.id == revision) {
            return not_found();
        }
        for entry in &mut candidate.revisions {
            if entry.id == revision {
                entry.state = ACTIVE;
            } else if entry.state == ACTIVE {
                entry.state = OBSOLETE;
            }
        }
        json_response(200, &json!({"Code": 1002}))
    }

    fn delete_revision(&mut self, link: &str, revision: &str) -> ResponseFuture {
        if let Some(candidate) = self.links.get_mut(link) {
            candidate
                .revisions
                .retain(|entry| entry.id != revision || entry.state == ACTIVE);
        }
        success()
    }

    fn revisions(&self, link: &str) -> ResponseFuture {
        let Some(candidate) = self.links.get(link) else {
            return not_found();
        };
        let revisions: Vec<Value> = candidate
            .revisions
            .iter()
            .map(|revision| {
                json!({
                    "ID": revision.id,
                    "State": revision.state,
                    "Size": self.revision_size(revision),
                    "CreateTime": NOW,
                })
            })
            .collect();
        json_response(200, &json!({"Code": 1000, "Revisions": revisions}))
    }

    fn revision(&self, link: &str, revision: &str, request: &Request) -> ResponseFuture {
        let Some(candidate) = self.links.get(link) else {
            return not_found();
        };
        let Some(revision) = candidate
            .revisions
            .iter()
            .find(|revision_entry| revision_entry.id == revision)
        else {
            return not_found();
        };
        let from: u32 = request
            .query("FromBlockIndex")
            .and_then(|from| from.parse().ok())
            .unwrap_or(1);
        let size: usize = request
            .query("PageSize")
            .and_then(|size| size.parse().ok())
            .unwrap_or(usize::MAX);
        let blocks: Vec<Value> = revision
            .blocks
            .range(from..)
            .take(size)
            .map(|(index, block)| {
                json!({
                    "Index": index,
                    "Hash": block.hash,
                    "Token": block.token,
                    "BareURL": self.storage(&block.token),
                    "EncSignature": block.signature,
                    "SignatureEmail": EMAIL,
                })
            })
            .collect();
        json_response(
            200,
            &json!({
                "Code": 1000,
                "Revision": {
                    "ID": revision.id,
                    "CreateTime": NOW,
                    "Size": self.revision_size(revision),
                    "ManifestSignature": revision.manifest_signature,
                    "SignatureEmail": EMAIL,
                    "State": revision.state,
                    "ChecksumVerified": revision.checksum_verified,
                    "XAttr": revision.attributes,
                    "Thumbnails": self.thumbnails_json(revision),
                    "Blocks": blocks,
                },
            }),
        )
    }

    fn revision_size(&self, revision: &Revision) -> usize {
        revision
            .blocks
            .values()
            .filter_map(|block| self.blobs.get(&block.token))
            .map(Vec::len)
            .sum()
    }

    fn thumbnails_json(&self, revision: &Revision) -> Vec<Value> {
        revision
            .thumbnails
            .iter()
            .filter_map(|thumbnail| {
                let sealed = self.blobs.get(&thumbnail.token)?;
                Some(json!({
                    "ThumbnailID": thumbnail.token,
                    "Type": thumbnail.kind,
                    "EncryptedSize": sealed.len(),
                    "Hash": base64_digest(sealed),
                }))
            })
            .collect()
    }

    fn thumbnail_addresses(&self, request: &Value) -> ResponseFuture {
        let mut thumbnails = Vec::new();
        let mut errors = Vec::new();
        for id in list(&request["ThumbnailIDs"])
            .iter()
            .filter_map(Value::as_str)
        {
            match self.blobs.contains_key(id) {
                true => thumbnails.push(json!({
                    "ThumbnailID": id,
                    "BareURL": self.storage(id),
                    "Token": id,
                })),
                false => errors.push(json!({
                    "ThumbnailID": id,
                    "Error": "Thumbnail does not exist",
                    "Code": 2501,
                })),
            }
        }
        json_response(
            200,
            &json!({"Code": 1000, "Thumbnails": thumbnails, "Errors": errors}),
        )
    }

    fn children(&self, folder: &str, anchor: Option<&str>) -> ResponseFuture {
        let mut listed: Vec<&Link> = self
            .links
            .values()
            .filter(|candidate| {
                candidate.volume == VOLUME
                    && candidate.parent.as_deref() == Some(folder)
                    && candidate.state == ACTIVE
            })
            .collect();
        listed.sort_unstable_by_key(|candidate| candidate.order);
        let ids: Vec<&str> = listed
            .iter()
            .map(|candidate| candidate.id.as_str())
            .collect();
        let start = anchor
            .and_then(|anchor| ids.iter().position(|id| *id == anchor))
            .map_or(0, |at| at + 1);
        let page: Vec<&str> = ids
            .iter()
            .skip(start)
            .take(CHILDREN_PAGE)
            .copied()
            .collect();
        let more = start + page.len() < ids.len();
        json_response(
            200,
            &json!({
                "Code": 1000,
                "LinkIDs": page,
                "AnchorID": page.last(),
                "More": i64::from(more),
            }),
        )
    }

    fn rename(&mut self, link: &str, request: &Value) -> ResponseFuture {
        let hash = request["Hash"].as_str().map(str::to_owned);
        let Some(parent) = self
            .links
            .get(link)
            .map(|candidate| candidate.parent.clone())
        else {
            return not_found();
        };
        if let (Some(parent), Some(hash)) = (&parent, &hash)
            && let Some(candidate) = self.occupant(parent, hash, Some(link))
        {
            return clash(&candidate.id, None);
        }
        if let Some(candidate) = self.links.get_mut(link) {
            candidate.name = text(&request["Name"]);
            if hash.is_some() {
                candidate.hash = hash;
            }
            candidate.modified += 1;
        }
        self.event(UPDATE_METADATA, link);
        success()
    }

    fn move_link(&mut self, link: &str, request: &Value) -> ResponseFuture {
        let parent = text(&request["ParentLinkID"]);
        let hash = text(&request["Hash"]);
        if !self.links.contains_key(link) {
            return not_found();
        }
        if let Some(candidate) = self.occupant(&parent, &hash, Some(link)) {
            return clash(&candidate.id, None);
        }
        if let Some(candidate) = self.links.get_mut(link) {
            candidate.parent = Some(parent);
            candidate.hash = Some(hash);
            candidate.name = text(&request["Name"]);
            candidate.node_passphrase = text(&request["NodePassphrase"]);
            if let Some(signature) = request["NodePassphraseSignature"].as_str() {
                candidate.node_passphrase_signature = signature.to_owned();
            }
            candidate.modified += 1;
        }
        self.event(UPDATE_METADATA, link);
        success()
    }

    fn available(&self, folder: &str, request: &Value) -> ResponseFuture {
        let available: Vec<&str> = list(&request["Hashes"])
            .iter()
            .filter_map(Value::as_str)
            .filter(|hash| self.occupant(folder, hash, None).is_none())
            .collect();
        json_response(
            200,
            &json!({"Code": 1000, "AvailableHashes": available, "PendingHashes": []}),
        )
    }

    fn set_state(&mut self, request: &Value, state: i64) -> ResponseFuture {
        let mut responses = Vec::new();
        for id in ids(request) {
            match self.links.get_mut(&id) {
                Some(candidate) => {
                    candidate.state = state;
                    candidate.modified += 1;
                    self.event(UPDATE, &id);
                    responses.push(json!({"LinkID": id, "Response": {"Code": 1000}}));
                }
                None => responses
                    .push(json!({"LinkID": id, "Response": {"Code": 2501, "Error": "Not found"}})),
            }
        }
        json_response(200, &json!({"Code": 1001, "Responses": responses}))
    }

    fn remove(&mut self, request: &Value, drafts: bool) -> ResponseFuture {
        let mut responses = Vec::new();
        for id in ids(request) {
            let removable = self
                .links
                .get(&id)
                .is_some_and(|candidate| !drafts || candidate.state == DRAFT);
            if removable {
                if !drafts {
                    self.event(DELETE, &id);
                }
                self.links.remove(&id);
                responses.push(json!({"LinkID": id, "Response": {"Code": 1000}}));
            } else {
                responses
                    .push(json!({"LinkID": id, "Response": {"Code": 2501, "Error": "Not found"}}));
            }
        }
        json_response(200, &json!({"Code": 1001, "Responses": responses}))
    }

    pub(crate) fn link_json(&self, link: &Link) -> Value {
        let mut body = json!({
            "Link": {
                "LinkID": link.id,
                "Type": link.kind,
                "ParentLinkID": link.parent,
                "State": link.state,
                "CreateTime": NOW,
                "ModifyTime": link.modified,
                "TrashTime": (link.state == TRASHED).then_some(link.modified),
                "Name": link.name,
                "NameHash": link.hash,
                "NodeKey": link.node_key,
                "NodePassphrase": link.node_passphrase,
                "NodePassphraseSignature": link.node_passphrase_signature,
                "SignatureEmail": EMAIL,
                "NameSignatureEmail": EMAIL,
                "OwnedBy": {"Email": EMAIL},
            },
            "Sharing": null,
            "Membership": null,
        });
        match link.kind {
            FOLDER_KIND => {
                body["Folder"] = json!({"NodeHashKey": link.hash_key, "XAttr": null});
            }
            ALBUM_KIND => {
                body["Album"] = json!({
                    "Hidden": false,
                    "Locked": false,
                    "CoverLinkID": null,
                    "LastActivityTime": NOW,
                    "PhotoCount": 0,
                    "NodeHashKey": link.hash_key,
                    "XAttr": null,
                });
            }
            _ => {
                let active = link
                    .revisions
                    .iter()
                    .find(|revision_entry| revision_entry.state == ACTIVE);
                let size = active.map_or(0, |revision| self.revision_size(revision));
                body["File"] = json!({
                    "ActiveRevision": active.map(|revision| json!({
                        "RevisionID": revision.id,
                        "CreateTime": NOW,
                        "EncryptedSize": size,
                        "ManifestSignature": revision.manifest_signature,
                        "SignatureEmail": EMAIL,
                        "XAttr": revision.attributes,
                        "Thumbnails": self.thumbnails_json(revision),
                        "ChecksumVerified": revision.checksum_verified,
                        "Photo": revision.photo,
                    })),
                    "TotalEncryptedSize": size,
                    "ContentKeyPacket": link.content_key_packet,
                    "ContentKeyPacketSignature": link.content_key_packet_signature,
                    "MediaType": link.media_type,
                });
            }
        }
        body
    }

    fn share_json(&self, share: &StoredShare) -> Value {
        if share.id == SHARE && self.shape.member_share {
            return self.fixture.member_share_json();
        }
        if share.id == SHARE {
            return self.fixture.owned_share_json(share.kind);
        }
        json!({
            "Code": 1000,
            "ShareID": share.id,
            "Type": share.kind,
            "State": 1,
            "LinkID": share.root,
            "VolumeID": VOLUME,
            "VolumeType": 1,
            "Creator": EMAIL,
            "Flags": 0,
            "Locked": false,
            "Key": share.key,
            "Passphrase": share.passphrase,
            "PassphraseSignature": share.passphrase_signature,
            "AddressID": ADDRESS_ID,
            "CreationTime": NOW,
            "Memberships": [],
        })
    }
}

impl Link {
    fn folder(
        id: &str,
        parent: Option<&str>,
        name: String,
        hash: Option<String>,
        node: &rikka_test_proton::NodeKey,
        hash_key: &str,
    ) -> Link {
        Link {
            id: id.to_owned(),
            volume: VOLUME,
            parent: parent.map(str::to_owned),
            kind: FOLDER_KIND,
            state: ACTIVE,
            name,
            hash,
            node_key: node.armored_key.clone(),
            node_passphrase: node.armored_passphrase.clone(),
            node_passphrase_signature: node.armored_signature.clone(),
            hash_key: Some(hash_key.to_owned()),
            content_key_packet: None,
            content_key_packet_signature: None,
            media_type: None,
            client_uid: None,
            revisions: Vec::new(),
            modified: NOW,
            order: 0,
            shared: false,
        }
    }

    pub(crate) fn sent(id: &str, parent: Option<String>, kind: i64, request: &Value) -> Link {
        Link {
            id: id.to_owned(),
            volume: VOLUME,
            parent,
            kind,
            state: ACTIVE,
            name: text(&request["Name"]),
            hash: request["Hash"].as_str().map(str::to_owned),
            node_key: text(&request["NodeKey"]),
            node_passphrase: text(&request["NodePassphrase"]),
            node_passphrase_signature: text(&request["NodePassphraseSignature"]),
            hash_key: None,
            content_key_packet: None,
            content_key_packet_signature: None,
            media_type: None,
            client_uid: None,
            revisions: Vec::new(),
            modified: NOW,
            order: 0,
            shared: false,
        }
    }

    pub(crate) fn with_hash_key(mut self, hash_key: String) -> Link {
        self.hash_key = Some(hash_key);
        self
    }

    fn sent_file(id: &str, parent: String, request: &Value) -> Link {
        Link {
            content_key_packet: Some(text(&request["ContentKeyPacket"])),
            content_key_packet_signature: request["ContentKeyPacketSignature"]
                .as_str()
                .map(str::to_owned),
            media_type: request["MIMEType"].as_str().map(str::to_owned),
            ..Link::sent(id, Some(parent), FILE_KIND, request)
        }
    }
}

impl Revision {
    fn draft(id: String, client_uid: Option<String>) -> Revision {
        Revision {
            id,
            state: DRAFT,
            client_uid,
            blocks: BTreeMap::new(),
            thumbnails: Vec::new(),
            manifest_signature: None,
            attributes: None,
            photo: None,
            checksum_verified: false,
        }
    }
}

pub(crate) fn volume() -> Value {
    json!({
        "VolumeID": VOLUME,
        "CreateTime": NOW,
        "UsedSpace": 0,
        "State": 1,
        "Type": 1,
        "Share": {"ShareID": SHARE, "LinkID": ROOT},
    })
}

fn device_json(device: &Device) -> Value {
    json!({
        "Device": {
            "DeviceID": device.id,
            "VolumeID": VOLUME,
            "SyncState": device.sync,
            "LastSyncTime": device.synced_at,
            "Type": device.kind,
            "CreateTime": NOW,
            "ModifyTime": NOW,
        },
        "Share": {"ShareID": device.share, "LinkID": device.root, "Name": device.name},
    })
}

fn event_id(number: usize) -> String {
    format!("event-{number}")
}

pub(crate) fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

pub(crate) fn list(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or_default()
}

fn ids(request: &Value) -> Vec<String> {
    list(&request["LinkIDs"])
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

pub(crate) fn refused(code: i32, details: Value) -> ResponseFuture {
    response(
        422,
        json!({"Code": code, "Error": "refused", "Details": details}).to_string(),
    )
}

fn clash(link: &str, draft: Option<(&str, Option<&str>)>) -> ResponseFuture {
    json_response(
        422,
        &json!({
            "Code": 2500,
            "Error": "A file or folder with that name already exists",
            "Details": {
                "ConflictLinkID": link,
                "ConflictDraftRevisionID": draft.map(|(revision, _)| revision),
                "ConflictDraftClientUID": draft.and_then(|(_, client)| client),
            },
        }),
    )
}

fn matches_route(pattern: &str, route: &str) -> bool {
    let mut expected = pattern.split('/');
    let mut actual = route.split('/');
    loop {
        match (expected.next(), actual.next()) {
            (None, None) => return true,
            (Some("*"), Some(_)) => {}
            (Some(wanted), Some(seen)) if wanted == seen => {}
            _ => return false,
        }
    }
}
