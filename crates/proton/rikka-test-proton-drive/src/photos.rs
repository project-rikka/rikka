use serde_json::{Value, json};

use rikka_test_proton::{
    ADDRESS_ID, EMAIL, Request, ResponseFuture, json_response, response, success,
};

use crate::digest::{lookup_hash, sha1_hex};
use crate::fixture::NOW;
use crate::ids::{
    ALBUM, CONTENT, COPIED, DUPLICATE, FILE, PHOTOS, ROOT, SHARE, SHOT_BURST, VOLUME,
};
use crate::state::{ALBUM_KIND, DriveState, Link, list, refused, text, volume};

const PHOTO_VOLUME: &str = "POST /drive/photos/volumes";
const COPY: &str = "POST /drive/volumes/elsewhere/links/shot/copy";

pub(crate) fn respond(state: &mut DriveState, request: &Request) -> Option<ResponseFuture> {
    let (method, path) = request.route();
    Some(match (method, path) {
        ("GET", "/drive/v2/shares/photos") => {
            if state.shape.photo_library_missing && state.request_count(PHOTO_VOLUME) == 0 {
                return Some(response(404, r#"{"Code":2501,"Error":"No photo volume"}"#));
            }
            json_response(
                200,
                &json!({
                    "Code": 1000,
                    "Volume": {"VolumeID": VOLUME, "UsedSpace": 0},
                    "Share": {"ShareID": SHARE, "CreatorEmail": EMAIL, "AddressID": ADDRESS_ID},
                }),
            )
        }
        ("POST", "/drive/photos/volumes") => {
            json_response(200, &json!({"Code": 1000, "Volume": volume()}))
        }
        ("GET", "/drive/volumes/volume/photos") => json_response(200, &timeline(request)),
        ("POST", "/drive/volumes/volume/photos/duplicates") => {
            json_response(200, &duplicates(&state.fixture.root_hash.secret))
        }
        ("POST", "/drive/photos/volumes/volume/albums") => create_album(state, &request.json()),
        ("GET", "/drive/photos/volumes/volume/albums") => json_response(200, &albums(request)),
        ("PUT", "/drive/photos/volumes/volume/albums/album") => success(),
        ("DELETE", "/drive/photos/volumes/volume/albums/album") => {
            refused(200_302, json!({"ChildLinkIDs": [FILE]}))
        }
        ("POST", "/drive/photos/volumes/volume/albums/album/add-multiple")
        | ("POST", "/drive/photos/volumes/volume/albums/album/remove-multiple")
        | ("PUT", "/drive/photos/volumes/volume/links/transfer-multiple") => {
            json_response(200, &outcomes(&request.json()))
        }
        ("POST", "/drive/volumes/elsewhere/links/shot/copy") => {
            if state.request_count(COPY) == 1 {
                return Some(refused(2000, json!({"Missing": [SHOT_BURST]})));
            }
            json_response(200, &json!({"Code": 1000, "LinkID": COPIED}))
        }
        ("POST", "/drive/photos/volumes/volume/links/file/tags")
        | ("DELETE", "/drive/photos/volumes/volume/links/file/tags")
        | ("POST", "/drive/photos/volumes/volume/links/file/favorite")
        | ("POST", "/drive/photos/volumes/elsewhere/links/shot/favorite") => success(),
        _ => return None,
    })
}

fn create_album(state: &mut DriveState, body: &Value) -> ResponseFuture {
    let sent = &body["Link"];
    state.insert(
        Link::sent(ALBUM, Some(ROOT.to_owned()), ALBUM_KIND, sent)
            .with_hash_key(text(&sent["NodeHashKey"])),
    );
    json_response(
        200,
        &json!({"Code": 1000, "Album": {"Link": {"LinkID": ALBUM}}}),
    )
}

fn outcomes(body: &Value) -> Value {
    let responses: Vec<Value> = ["AlbumData", "Links"]
        .iter()
        .filter_map(|field| body[*field].as_array())
        .flatten()
        .map(|entry| entry["LinkID"].clone())
        .chain(list(&body["LinkIDs"]).iter().cloned())
        .map(|id| json!({"LinkID": id, "Response": {"Code": 1000}}))
        .collect();

    json!({"Code": 1000, "Responses": responses})
}

fn timeline(request: &Request) -> Value {
    let size = request
        .query("PageSize")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(PHOTOS.len());
    let start = request
        .query("PreviousPageLastLinkID")
        .and_then(|last| PHOTOS.iter().position(|id| *id == last))
        .map_or(0, |at| at + 1);
    let photos: Vec<Value> = PHOTOS
        .iter()
        .enumerate()
        .skip(start)
        .take(size)
        .map(|(at, id)| {
            let first = at == 0;
            json!({
                "LinkID": id,
                "CaptureTime": NOW - i64::try_from(at).unwrap_or_default() * 60,
                "Hash": format!("name-{id}"),
                "ContentHash": format!("content-{id}"),
                "Tags": if first { vec![0, 3] } else { Vec::new() },
                "RelatedPhotos": if first {
                    vec![json!({"LinkID": "p1-motion", "CaptureTime": NOW, "Hash": "h", "ContentHash": "c"})]
                } else {
                    Vec::new()
                },
            })
        })
        .collect();

    json!({"Code": 1000, "Photos": photos})
}

fn duplicates(root_hash: &[u8]) -> Value {
    let name = lookup_hash(root_hash, "beach.jpg");
    let content = lookup_hash(root_hash, &sha1_hex(CONTENT));
    json!({
        "Code": 1000,
        "DuplicateHashes": [
            {"Hash": name, "ContentHash": content, "LinkState": 1, "LinkID": DUPLICATE, "RevisionID": "r", "ClientUID": null},
            {"Hash": name, "ContentHash": "another-content", "LinkState": 1, "LinkID": "cousin", "RevisionID": "r", "ClientUID": null},
            {"Hash": name, "ContentHash": content, "LinkState": 2, "LinkID": "binned", "RevisionID": "r", "ClientUID": null},
        ],
    })
}

fn albums(request: &Request) -> Value {
    match request.query("AnchorID") {
        None => json!({
            "Code": 1000,
            "Albums": [{"LinkID": "first", "VolumeID": VOLUME, "PhotoCount": 3, "LastActivityTime": NOW, "Locked": false, "ShareID": null, "CoverLinkID": "p1"}],
            "AnchorID": "next",
            "More": true,
        }),
        Some(_) => json!({
            "Code": 1000,
            "Albums": [{"LinkID": "second", "VolumeID": VOLUME, "PhotoCount": 0, "LastActivityTime": NOW + 100, "Locked": false, "ShareID": null, "CoverLinkID": null}],
            "AnchorID": null,
            "More": false,
        }),
    }
}
