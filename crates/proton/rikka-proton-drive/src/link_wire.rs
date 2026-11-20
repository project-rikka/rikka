use jiff::Timestamp;
use serde::Deserialize;

use crate::identifier::LinkId;
use crate::link::{ActiveRevision, FileProperties, FolderProperties, Link, LinkState, LinkType};
use crate::photo_properties::{AlbumProperties, PhotoInfo, PhotoProperties};
use crate::role::Role;
use crate::sharing_details::SharingDetails;

#[derive(Debug, Deserialize)]
pub(crate) struct LinkDetails {
    #[serde(rename = "Link")]
    link: LinkDto,
    #[serde(rename = "File", default)]
    file: Option<FileDto>,
    #[serde(rename = "Folder", default)]
    folder: Option<FolderDto>,
    #[serde(rename = "Album", default)]
    album: Option<AlbumDto>,
    #[serde(rename = "Photo", default)]
    photo: Option<PhotoFileDto>,
    #[serde(rename = "Sharing", default)]
    sharing: Option<SharingDetails>,
    #[serde(rename = "Membership", default)]
    membership: Option<MembershipDto>,
}

#[derive(Debug, Deserialize)]
struct LinkDto {
    #[serde(rename = "LinkID")]
    id: LinkId,
    #[serde(rename = "Type", default)]
    kind: LinkType,
    #[serde(rename = "ParentLinkID", default)]
    parent_id: Option<LinkId>,
    #[serde(rename = "State", default)]
    state: LinkState,
    #[serde(
        rename = "CreateTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    created_at: Option<Timestamp>,
    #[serde(
        rename = "ModifyTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    modified_at: Option<Timestamp>,
    #[serde(
        rename = "TrashTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    trashed_at: Option<Timestamp>,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "NameHash", default)]
    hash: Option<String>,
    #[serde(rename = "NodeKey")]
    node_key: String,
    #[serde(rename = "NodePassphrase")]
    node_passphrase: String,
    #[serde(rename = "NodePassphraseSignature", default)]
    node_passphrase_signature: Option<String>,
    #[serde(rename = "SignatureEmail", default)]
    signature_email: Option<String>,
    #[serde(rename = "NameSignatureEmail", default)]
    name_signature_email: Option<String>,
    #[serde(rename = "OwnedBy", default)]
    owned_by: Option<OwnedBy>,
    #[serde(rename = "DirectPermissions", default)]
    direct_permissions: Option<Role>,
}

#[derive(Debug, Deserialize)]
struct OwnedBy {
    #[serde(rename = "Email", default)]
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FileDto {
    #[serde(rename = "ActiveRevision", default)]
    active_revision: Option<ActiveRevision>,
    #[serde(rename = "TotalEncryptedSize", default)]
    total_size: i64,
    #[serde(rename = "ContentKeyPacket", default)]
    content_key_packet: Option<String>,
    #[serde(rename = "ContentKeyPacketSignature", default)]
    content_key_packet_signature: Option<String>,
    #[serde(rename = "MediaType", default)]
    media_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FolderDto {
    #[serde(rename = "NodeHashKey", default)]
    node_hash_key: Option<String>,
    #[serde(rename = "XAttr", default)]
    extended_attributes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AlbumDto {
    #[serde(flatten)]
    properties: AlbumProperties,
    #[serde(rename = "XAttr", default)]
    extended_attributes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PhotoFileDto {
    #[serde(flatten)]
    file: FileDto,
    #[serde(flatten)]
    info: PhotoInfo,
    #[serde(flatten)]
    properties: PhotoProperties,
}

#[derive(Debug, Deserialize)]
struct MembershipDto {
    #[serde(rename = "Permissions", default)]
    permissions: Option<Role>,
}

impl From<LinkDetails> for Link {
    fn from(details: LinkDetails) -> Link {
        let LinkDetails {
            link,
            file,
            folder,
            album,
            photo,
            sharing,
            membership,
        } = details;

        let (file, photo_info, photo) = match (file, photo) {
            (_, Some(photo)) => (Some(photo.file), Some(photo.info), Some(photo.properties)),
            (file, None) => (file, None, None),
        };
        let media_type = file
            .as_ref()
            .and_then(|file| file.media_type.clone())
            .unwrap_or_default();
        let size = file.as_ref().map_or(0, |file| file.total_size);
        let extended_attributes = match (&file, &folder, &album) {
            (Some(file), _, _) => file
                .active_revision
                .as_ref()
                .and_then(|revision| revision.extended_attributes().map(str::to_owned)),
            (None, Some(folder), _) => folder.extended_attributes.clone(),
            (None, None, Some(album)) => album.extended_attributes.clone(),
            (None, None, None) => None,
        };

        Link {
            id: link.id,
            parent_id: link.parent_id,
            volume_id: None,
            kind: link.kind,
            state: link.state,
            name: link.name,
            hash: link.hash.filter(|hash| !hash.is_empty()),
            media_type,
            size,
            node_key: link.node_key,
            node_passphrase: link.node_passphrase,
            node_passphrase_signature: link.node_passphrase_signature,
            signature_email: link.signature_email,
            name_signature_email: link.name_signature_email,
            extended_attributes,
            file: file.map(|file| FileProperties {
                content_key_packet: file.content_key_packet,
                content_key_packet_signature: file.content_key_packet_signature,
                active_revision: file
                    .active_revision
                    .map(|revision| revision.with_photo(photo_info)),
            }),
            folder: folder.map(|folder| FolderProperties {
                node_hash_key: folder.node_hash_key,
            }),
            album: album.map(|album| album.properties),
            photo,
            sharing,
            permissions: link
                .direct_permissions
                .or(membership.and_then(|membership| membership.permissions)),
            owner: link.owned_by.and_then(|owner| owner.email),
            created_at: link.created_at,
            modified_at: link.modified_at,
            trashed_at: link.trashed_at,
        }
    }
}
