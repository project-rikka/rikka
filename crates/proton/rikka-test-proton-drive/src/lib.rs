mod digest;
mod fixture;
mod ids;
mod photos;
mod shape;
mod sharing;
mod state;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rikka_proton_account::Account;
use rikka_proton_drive::DriveClient;
use rikka_proton_pgp::{Pgp, PrivateKey, SessionKey};
use rikka_test_proton::{
    FakeSrp, NodeKey, Owner, PASSPHRASE, Request, Response, ResponseFuture, SIGNED_MODULUS,
    builder, json_response, serve,
};
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::fixture::Fixture;
use crate::state::DriveState;

pub use digest::{base64_digest, lookup_hash, sha1_hex};
pub use ids::{
    ALBUM, BOOKMARKED_NAME, BOOKMARKED_PASSWORD, CONTENT, COPIED, DEVICE, DOCUMENT, DRAFT_NAME,
    DUPLICATE, ELSEWHERE, EXTERNAL_INVITEE, EXTRA_FOLDER_PREFIX, FILE, FILE_NAME, FOLDER,
    FOLDER_NAME, FOREIGN_OWNER, INVITATION, INVITEE, MODULUS_ID, OLDER_REVISION, PHOTOS,
    PUBLIC_URL, REVISION, ROOT, SHARE, SHARED_NAME, SHOT, SHOT_BURST, SHOT_MOTION, SPLIT, STALE,
    THUMBNAIL, TOKEN, VOLUME, extra_folder_id,
};
pub use shape::{Conflict, Corruption, Shape};

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct DummyDrive {
    address: String,
    fixture: Arc<Fixture>,
    state: Arc<Mutex<DriveState>>,
    released: Arc<watch::Sender<u64>>,
    block_uploads: Arc<watch::Sender<usize>>,
}

impl DummyDrive {
    pub async fn start() -> DummyDrive {
        DummyDrive::with_shape(Shape::empty()).await
    }

    pub async fn with_shape(shape: Shape) -> DummyDrive {
        let fixture = Arc::new(Fixture::new());
        let state = Arc::new(Mutex::new(DriveState::new(Arc::clone(&fixture), shape)));
        let released = Arc::new(watch::Sender::new(0));
        let block_uploads = Arc::new(watch::Sender::new(0));
        let srp: Arc<Mutex<Option<FakeSrp>>> = Arc::default();
        let (state_handle, release_handle, uploads_handle) = (
            Arc::clone(&state),
            Arc::clone(&released),
            Arc::clone(&block_uploads),
        );
        let address = serve(move |request| -> ResponseFuture {
            let mut state = state_handle.lock().unwrap();
            if let Some(upload) = state.record_block_upload(&request) {
                uploads_handle.send_replace(upload);
                if state.is_stalled(upload) {
                    let mut release = release_handle.subscribe();
                    return Box::pin(async move {
                        release.changed().await.ok();
                        Response::json(503, &json!({"Code": 503, "Error": "stalled"}))
                    });
                }
            }
            respond(&mut state, &mut srp.lock().unwrap(), &request)
        })
        .await;
        state.lock().unwrap().set_base(address.clone());

        DummyDrive {
            address,
            fixture,
            state,
            released,
            block_uploads,
        }
    }

    pub async fn client(&self) -> DriveClient {
        let api = builder(&self.address).timeout(TIMEOUT).build().unwrap();
        let account = Account::with_pgp(api, self.fixture.pgp);
        let keys = account.unlock_with_passphrase(PASSPHRASE).await.unwrap();

        DriveClient::for_account(&account, &Arc::new(keys))
    }

    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn stall_uploads_after(&self, blocks: usize) {
        self.state.lock().unwrap().stall_uploads_after(Some(blocks));
    }

    pub fn release_uploads(&self) {
        self.state.lock().unwrap().stall_uploads_after(None);
        self.released.send_modify(|generation| *generation += 1);
    }

    #[must_use]
    pub fn block_uploads(&self) -> usize {
        *self.block_uploads.borrow()
    }

    pub async fn wait_for_block_uploads(&self, count: usize) {
        self.block_uploads
            .subscribe()
            .wait_for(|uploads| *uploads >= count)
            .await
            .unwrap();
    }

    #[must_use]
    pub fn request_count(&self, route: &str) -> usize {
        self.state.lock().unwrap().request_count(route)
    }

    #[must_use]
    pub fn bodies(&self, route: &str) -> Vec<Value> {
        self.state.lock().unwrap().bodies(route)
    }

    #[must_use]
    pub fn last_body(&self, route: &str) -> Value {
        self.bodies(route).pop().unwrap_or(Value::Null)
    }

    #[must_use]
    pub fn uploaded(&self, index: u32) -> Vec<u8> {
        self.state.lock().unwrap().uploaded(index)
    }

    #[must_use]
    pub fn max_concurrent_block_reads(&self) -> usize {
        self.state.lock().unwrap().max_concurrent_reads()
    }

    #[must_use]
    pub fn pgp(&self) -> Pgp {
        self.fixture.pgp
    }

    #[must_use]
    pub fn owner(&self) -> &Owner {
        &self.fixture.owner
    }

    #[must_use]
    pub fn share(&self) -> &NodeKey {
        &self.fixture.share
    }

    #[must_use]
    pub fn root(&self) -> &NodeKey {
        &self.fixture.root
    }

    #[must_use]
    pub fn folder(&self) -> &NodeKey {
        &self.fixture.folder
    }

    #[must_use]
    pub fn file(&self) -> &NodeKey {
        &self.fixture.file
    }

    #[must_use]
    pub fn root_hash(&self) -> &[u8] {
        &self.fixture.root_hash.secret
    }

    #[must_use]
    pub fn content(&self) -> &SessionKey {
        &self.fixture.content
    }

    #[must_use]
    pub fn verification(&self) -> [u8; 32] {
        self.fixture.verification
    }

    #[must_use]
    pub fn invitee_key(&self) -> &PrivateKey {
        &self.fixture.invitee_key
    }

    #[must_use]
    pub fn share_session_key(&self) -> SessionKey {
        self.fixture.share_session_key()
    }

    #[must_use]
    pub fn decrypt_text(&self, key: &PrivateKey, armored: &str) -> String {
        let plaintext = self
            .fixture
            .pgp
            .decrypt()
            .with_key(key.clone())
            .message(armored)
            .unwrap();
        String::from_utf8(plaintext.into_data()).unwrap()
    }
}

impl std::fmt::Debug for DummyDrive {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DummyDrive")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

fn respond(state: &mut DriveState, srp: &mut Option<FakeSrp>, request: &Request) -> ResponseFuture {
    state.record(request);
    match request.route() {
        ("POST", "/auth/v4/sessions") => {
            return json_response(
                200,
                &json!({
                    "Code": 1000,
                    "UID": "session",
                    "AccessToken": "access",
                    "RefreshToken": "refresh",
                    "Scopes": [],
                }),
            );
        }
        ("GET", "/auth/v4/modulus") => {
            return json_response(
                200,
                &json!({"Code": 1000, "Modulus": SIGNED_MODULUS, "ModulusID": MODULUS_ID}),
            );
        }
        _ => {}
    }
    if let Some(response) = state.fixture.owner.routes(request) {
        return response;
    }
    sharing::respond(state, srp, request)
        .or_else(|| photos::respond(state, request))
        .unwrap_or_else(|| state.respond_core(request))
}
