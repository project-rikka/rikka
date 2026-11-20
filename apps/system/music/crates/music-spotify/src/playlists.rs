use std::collections::HashSet;

use music_core::{Artwork, Id, Picture, Playlist, PlaylistPage, Result};
use music_spotify_api::{Capabilities, ListAttributes, MetaItem, SelectedListContent, SpClient};

use crate::failure;
use crate::metadata;
use crate::uri::{self, PLAYLIST, TRACK};

pub(crate) async fn owned(api: &SpClient) -> Result<Vec<Playlist>> {
    Ok(rootlist(api)
        .await?
        .iter()
        .flat_map(|part| listed(part, api.username()))
        .collect())
}

pub(crate) async fn page(api: &SpClient, id: &Id) -> Result<PlaylistPage> {
    let content = content(api, id).await?;
    let uris = track_uris(&content);
    let tracks = metadata::tracks(api, &uris).await?;
    let attributes = content.attributes.clone().unwrap_or_default();
    Ok(PlaylistPage {
        playlist: Playlist {
            id: id.clone(),
            name: attributes.name.clone().unwrap_or_default(),
            owner: content.owner_username.clone(),
            description: attributes.description.clone().unwrap_or_default(),
            artwork: artwork(&attributes),
            length: usize::try_from(content.length).unwrap_or(tracks.len()),
            editable: editable(
                content.capabilities.as_ref(),
                &attributes,
                &content.owner_username,
                api.username(),
            ),
        },
        tracks: tracks.into(),
    })
}

pub(crate) async fn item_uris(api: &SpClient, id: &Id) -> Result<Vec<String>> {
    let content = content(api, id).await?;
    Ok(content
        .contents
        .map(|contents| contents.items.into_iter().map(|item| item.uri).collect())
        .unwrap_or_default())
}

pub(crate) async fn tracks(api: &SpClient, id: &Id) -> Result<Vec<music_core::Track>> {
    let content = content(api, id).await?;
    metadata::tracks(api, &track_uris(&content)).await
}

pub(crate) async fn followed(api: &SpClient) -> Result<HashSet<String>> {
    Ok(rootlist(api)
        .await?
        .into_iter()
        .filter_map(|part| part.contents)
        .flat_map(|contents| contents.items)
        .map(|item| item.uri)
        .collect())
}

async fn rootlist(api: &SpClient) -> Result<Vec<SelectedListContent>> {
    api.rootlist().await.map_err(failure::api)
}

async fn content(api: &SpClient, id: &Id) -> Result<SelectedListContent> {
    api.playlist(uri::spotify_id(id)?)
        .await
        .map_err(failure::api)
}

fn track_uris(content: &SelectedListContent) -> Vec<String> {
    content
        .contents
        .iter()
        .flat_map(|contents| &contents.items)
        .map(|item| item.uri.clone())
        .filter(|uri| uri.starts_with(TRACK))
        .collect()
}

fn listed(rootlist: &SelectedListContent, username: &str) -> Vec<Playlist> {
    let Some(contents) = rootlist.contents.as_ref() else {
        return Vec::new();
    };
    let unnamed = ListAttributes::default();
    contents
        .items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let id = uri::id_in(&item.uri, PLAYLIST)?;
            let meta = contents.meta_items.get(index);
            let attributes = meta
                .and_then(|meta| meta.attributes.as_ref())
                .unwrap_or(&unnamed);
            let owner = meta.map_or("", |meta: &MetaItem| meta.owner_username.as_str());
            Some(Playlist {
                id,
                name: attributes.name.clone().unwrap_or_default(),
                owner: owner.to_owned(),
                description: attributes.description.clone().unwrap_or_default(),
                artwork: artwork(attributes),
                length: meta
                    .and_then(|meta| usize::try_from(meta.length).ok())
                    .unwrap_or(0),
                editable: editable(
                    meta.and_then(|meta| meta.capabilities.as_ref()),
                    attributes,
                    owner,
                    username,
                ),
            })
        })
        .collect()
}

fn editable(
    capabilities: Option<&Capabilities>,
    attributes: &ListAttributes,
    owner: &str,
    username: &str,
) -> bool {
    match capabilities.and_then(|capabilities| capabilities.can_edit_items) {
        Some(can_edit) => can_edit,
        None => owner == username || attributes.collaborative,
    }
}

fn artwork(attributes: &ListAttributes) -> Artwork {
    let mut pictures: Vec<Picture> = attributes
        .picture_size
        .iter()
        .filter(|size| size.url.starts_with("https://"))
        .map(|size| Picture {
            url: size.url.clone(),
            width: match size.target_name.as_str() {
                "small" => 60,
                "large" => 640,
                "xlarge" => 1280,
                _ => 300,
            },
        })
        .collect();
    if let Some(url) = uri::image(&attributes.picture) {
        pictures.push(Picture { url, width: 300 });
    }
    Artwork::new(pictures)
}
