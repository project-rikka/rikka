pub const VOLUME: &str = "volume";
pub const ELSEWHERE: &str = "elsewhere";
pub const SHARE: &str = "share";
pub const ROOT: &str = "root";
pub const FOLDER: &str = "folder";
pub const FILE: &str = "file";
pub const REVISION: &str = "revision";
pub const OLDER_REVISION: &str = "older";
pub(crate) const UNFINISHED_REVISION: &str = "unfinished";
pub const STALE: &str = "stale";
pub(crate) const STALE_DRAFT: &str = "stale-draft";
pub const ALBUM: &str = "album";
pub const DOCUMENT: &str = "document";
pub const DEVICE: &str = "device";
pub const SHOT: &str = "shot";
pub const SHOT_MOTION: &str = "shot-motion";
pub const SHOT_BURST: &str = "shot-burst";
pub const COPIED: &str = "copied";
pub const DUPLICATE: &str = "duplicate";
pub const PHOTOS: [&str; 5] = ["p1", "p2", "p3", "p4", "p5"];
pub const MODULUS_ID: &str = "modulus";
pub(crate) const KNOWN_THUMBNAIL: &str = "thumbnail";

pub const FOLDER_NAME: &str = "photos";
pub const FILE_NAME: &str = "holiday.txt";
pub const DRAFT_NAME: &str = "draft.txt";
pub const CONTENT: &[u8] = b"the first block and then the second block";
pub const THUMBNAIL: &[u8] = b"a small picture of the holiday";
pub const SPLIT: usize = 21;
pub const EXTRA_FOLDER_PREFIX: &str = "extra_folders";

pub const INVITEE: &str = "invitee_key@proton.test";
pub const EXTERNAL_INVITEE: &str = "outsider@example.test";
pub const FOREIGN_OWNER: &str = "owner@elsewhere.test";
pub const TOKEN: &str = "token";
pub const PUBLIC_URL: &str = "https://drive.proton.me/urls/token";
pub const INVITATION: &str = "invitation";
pub const SHARED_NAME: &str = "Shared trip";
pub const BOOKMARKED_PASSWORD: &str = "ABCDEFGHIJKLsecret";
pub const BOOKMARKED_NAME: &str = "Saved trip";

#[must_use]
pub fn extra_folder_id(at: usize) -> String {
    format!("{EXTRA_FOLDER_PREFIX}-{at:03}")
}
