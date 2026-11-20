use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use music_blend::BlendId;
use music_core::{AlbumPage, ArtistPage, Card, Catalog, Found, Home, Id, Playlist, Shelf, Track};
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::blocks::{
    CARD, Header, Leading, Listening, ROW, Within, columns, header, heading, track_row, waiting,
};
use crate::gallery::{Cards, Gallery, Items};
use crate::library::Load;
use crate::local;
use crate::menus::Subject;
use crate::music::Music;
use crate::nav::Place;
use crate::wording::{release, songs};

const POPULAR: usize = 5;

struct Source {
    account: String,
    catalog: Arc<dyn Catalog>,
}

pub(crate) enum Block {
    Header(Box<Header>),
    Heading(Label),
    Columns,
    Tracks {
        list: Arc<[Track]>,
        span: Range<usize>,
        leading: Leading,
        within: Option<Within>,
    },
    Shelf(Gallery),
    Line(Rc<dyn Fn() -> Element>),
    Notice(MaterialSymbol, Label),
}

impl Block {
    pub(crate) fn tracks(list: &Arc<[Track]>, leading: Leading) -> Block {
        Block::Tracks {
            list: Arc::clone(list),
            span: 0..list.len(),
            leading,
            within: None,
        }
    }

    pub(crate) fn line(line: impl Fn() -> Element + 'static) -> Block {
        Block::Line(Rc::new(line))
    }

    fn rows(&self) -> usize {
        match self {
            Block::Tracks { span, .. } => span.len(),
            Block::Shelf(_) => 2,
            _ => 1,
        }
    }

    fn view(&self, row: usize, first: bool, listening: &Listening) -> Element {
        match self {
            Block::Header(page_header) => header((**page_header).clone()),
            Block::Heading(title) => heading(title.clone(), above(first)),
            Block::Columns => columns().any(),
            Block::Tracks {
                list,
                span,
                leading,
                within,
            } => track_row(list, span.start + row, *leading, within.as_ref(), listening),
            Block::Shelf(gallery) if row == 0 => gallery.heading(above(first)),
            Block::Shelf(gallery) => gallery.preview(listening),
            Block::Line(line) => line(),
            Block::Notice(symbol, message) => status_page(*symbol, message.clone()).fill().any(),
        }
    }
}

fn above(first: bool) -> Space {
    match first {
        true => Space::ROOMY,
        false => Space::LOOSE,
    }
}

pub(crate) fn page_of(blocks: Vec<Block>, listening: Listening) -> Element {
    let starts: Vec<usize> = blocks
        .iter()
        .scan(0, |start, block| {
            let at = *start;
            *start += block.rows();
            Some(at)
        })
        .collect();
    let rows = blocks.iter().map(Block::rows).sum();
    lazy_column(rows, move |slot| {
        let block = starts.partition_point(|start| *start <= slot.index) - 1;
        blocks[block].view(slot.index - starts[block], slot.index == 0, &listening)
    })
    .estimate(ROW)
    .padding(Inset::edges(
        Space::NONE,
        Space::LOOSE,
        Space::WIDE,
        Space::LOOSE,
    ))
    .fill()
    .any()
}

pub(crate) fn view(music: &Music) -> impl View {
    let listening = Listening::new(music);
    let Some(catalog) = music.catalog() else {
        return waiting();
    };
    let current = music.nav().current().clone();
    if let Some(page) = local::page(music, &current, &listening) {
        return page;
    }
    let source = Source {
        account: music.account_key().to_owned(),
        catalog,
    };
    match current {
        Place::Home => home(source, listening),
        Place::Search => search(source, music.search_query().to_owned(), listening),
        Place::Liked => liked(music, listening),
        Place::Playlist(id) => playlist(music, &id, listening),
        Place::Blend(id) => blend(music, &id, listening),
        Place::Album(id) => album(source, id, listening),
        Place::Artist(id) => artist(source, id, listening),
        Place::Mix(seed) => mix(source, seed, listening),
        Place::Gallery(gallery) => match gallery.items {
            Items::Tracks(tracks) => songs_of(gallery.title, &tracks, listening),
            Items::Cards(cards) => cards_of(gallery.title, cards),
        },
        Place::Albums
        | Place::Artists
        | Place::Songs
        | Place::Genres
        | Place::Folders
        | Place::Genre { .. }
        | Place::Folder { .. } => {
            status_page(MaterialSymbol::LIBRARY_MUSIC, tr!("music.local.missing"))
                .fill()
                .any()
        }
    }
}

pub(crate) fn fetched<T: Send + Sync + 'static>(
    fetch: Fetch<T>,
    ready: impl Fn(&T) -> Element + 'static,
) -> Element {
    container(
        async_view(fetch, ready)
            .loading(waiting)
            .failed(|why| status_page(MaterialSymbol::CLOUD_OFF, why.to_owned()).fill()),
    )
    .fill()
    .any()
}

fn home(source: Source, listening: Listening) -> Element {
    let catalog = source.catalog;
    let loading = fetch(("music-home", source.account), move || async move {
        catalog.home().await
    });
    fetched(loading, move |home: &Home| {
        let mut blocks: Vec<Block> = home.shelves.iter().map(shelved).collect();
        if blocks.is_empty() {
            blocks.push(Block::Notice(
                MaterialSymbol::LIBRARY_MUSIC,
                tr!("music.home.empty"),
            ));
        }
        page_of(blocks, listening.clone())
    })
}

fn shelved(shelf: &Shelf) -> Block {
    let mixed =
        |title: Label, cards: &Arc<[Card]>| Gallery::cards(title, Cards::Mixed(Arc::clone(cards)));
    Block::Shelf(match shelf {
        Shelf::QuickPicks(tracks) => Gallery::tracks(tr!("music.home.picks"), tracks),
        Shelf::RecentlyPlayed(tracks) => Gallery::tracks(tr!("music.home.recent"), tracks),
        Shelf::Following(tracks) => Gallery::tracks(tr!("music.home.following"), tracks),
        Shelf::Favourites(tracks) => Gallery::tracks(tr!("music.home.favourites"), tracks),
        Shelf::Songs { title, tracks } => Gallery::tracks(title.clone(), tracks),
        Shelf::MixesFor(cards) => mixed(tr!("music.home.mixes"), cards),
        Shelf::RecentAlbums(cards) => mixed(tr!("music.home.albums"), cards),
        Shelf::RecentlyAdded(cards) => mixed(tr!("music.home.added"), cards),
        Shelf::YourPlaylists(cards) => mixed(tr!("music.home.playlists"), cards),
        Shelf::Artists(cards) => mixed(tr!("music.home.artists"), cards),
        Shelf::NewReleases(cards) => mixed(tr!("music.home.new"), cards),
        Shelf::Acclaimed(cards) => mixed(tr!("music.home.acclaimed"), cards),
        Shelf::MostPlayed(cards) => mixed(tr!("music.home.popular"), cards),
        Shelf::Curated(cards) => mixed(tr!("music.home.curated"), cards),
        Shelf::Titled { title, cards } => mixed(Label::from(title.clone()), cards),
    })
}

fn search(source: Source, query_text: String, listening: Listening) -> Element {
    let catalog = source.catalog;
    if query_text.is_empty() {
        return status_page(MaterialSymbol::SEARCH, tr!("music.search.hint"))
            .fill()
            .any();
    }
    let query = query_text.clone();
    let loading = fetch(
        ("music-search", source.account, query_text),
        move || async move { catalog.search(&query).await },
    );
    fetched(loading, move |results: &Found| {
        if results.is_empty() {
            return status_page(MaterialSymbol::SEARCH_OFF, tr!("music.search.none"))
                .fill()
                .any();
        }
        let mut blocks = Vec::new();
        if !results.tracks.is_empty() {
            blocks.push(Block::Heading(tr!("music.search.songs")));
            blocks.push(Block::tracks(&results.tracks, Leading::Cover));
        }
        if !results.artists.is_empty() {
            let artists = Cards::Artists(Arc::clone(&results.artists));
            shelved_cards(&mut blocks, tr!("music.search.artists"), artists);
        }
        if !results.albums.is_empty() {
            let albums = Cards::Albums(Arc::clone(&results.albums));
            shelved_cards(&mut blocks, tr!("music.search.albums"), albums);
        }
        if !results.playlists.is_empty() {
            let playlists = Cards::Playlists(Arc::clone(&results.playlists));
            shelved_cards(&mut blocks, tr!("music.search.playlists"), playlists);
        }
        page_of(blocks, listening.clone())
    })
}

fn shelved_cards(blocks: &mut Vec<Block>, title: Label, cards: Cards) {
    blocks.push(Block::Shelf(Gallery::cards(title, cards)));
}

fn listed(header: Header, leading: Leading, within: Option<Within>) -> Vec<Block> {
    let list = Arc::clone(&header.tracks);
    vec![
        Block::Header(Box::new(header)),
        Block::Columns,
        Block::Tracks {
            span: 0..list.len(),
            list,
            leading,
            within,
        },
    ]
}

fn liked(music: &Music, listening: Listening) -> Element {
    match music.library().songs() {
        Load::Loading => waiting(),
        Load::Failed(why) => status_page(MaterialSymbol::CLOUD_OFF, why.clone())
            .fill()
            .any(),
        Load::Ready(list) => {
            let header = Header {
                artwork: Default::default(),
                round: false,
                kind: tr!("music.kind.collection"),
                title: tr!("music.songs.title").to_string(),
                lines: vec![songs(list.len()).to_string()],
                fidelity: None,
                tracks: Arc::clone(list),
                menu: None,
            };
            let blocks = match list.is_empty() {
                true => vec![
                    Block::Header(Box::new(header)),
                    Block::Notice(MaterialSymbol::FAVORITE, tr!("music.songs.empty")),
                ],
                false => listed(header, Leading::Cover, None),
            };
            page_of(blocks, listening)
        }
    }
}

fn playlist(music: &Music, id: &Id, listening: Listening) -> Element {
    let library = music.library();
    let page = match library.page(id) {
        Load::Loading => return waiting(),
        Load::Failed(why) => return status_page(MaterialSymbol::CLOUD_OFF, why).fill().any(),
        Load::Ready(page) => page,
    };
    let editable = library.can_edit(&page.playlist);
    let mut lines = Vec::new();
    if !page.playlist.description.is_empty() {
        lines.push(page.playlist.description.clone());
    }
    let counted = songs(page.tracks.len());
    lines.push(match page.playlist.owner.is_empty() {
        true => counted.to_string(),
        false => format!("{} · {counted}", page.playlist.owner),
    });
    let header = Header {
        artwork: page.playlist.artwork.clone(),
        round: false,
        kind: tr!("music.kind.playlist"),
        title: page.playlist.name.clone(),
        lines,
        fidelity: None,
        tracks: Arc::clone(&page.tracks),
        menu: Some(Subject::Playlist(Playlist {
            editable,
            ..page.playlist.clone()
        })),
    };
    let blocks = match page.tracks.is_empty() {
        true => vec![
            Block::Header(Box::new(header)),
            Block::Notice(MaterialSymbol::QUEUE_MUSIC, tr!("music.playlist.empty")),
        ],
        false => listed(
            header,
            Leading::Cover,
            editable.then(|| Within::Playlist(id.clone())),
        ),
    };
    page_of(blocks, listening)
}

fn blend(music: &Music, id: &BlendId, listening: Listening) -> Element {
    let blends = music.blends();
    let Some(blend) = blends.find(id) else {
        return status_page(MaterialSymbol::QUEUE_MUSIC, tr!("music.blend.gone"))
            .fill()
            .any();
    };
    let tracks: Arc<[Track]> = blend.tracks().into();
    let mut lines = vec![songs(tracks.len()).to_string()];
    lines.push(saved_in(blend.mirrors()));
    if blends.is_syncing(id) {
        lines.push(tr!("music.blend.syncing").to_string());
    } else if let Some(account_errors) = blends.account_errors_of(id) {
        let names: Vec<&str> = account_errors
            .iter()
            .map(|account_error| account_error.service.name)
            .collect();
        lines.push(tr!("music.blend.unsynced", names = names.join(", ").as_str()).to_string());
    }
    let header = Header {
        artwork: blend.artwork(),
        round: false,
        kind: tr!("music.kind.blend"),
        title: blend.name().to_owned(),
        lines,
        fidelity: None,
        tracks: Arc::clone(&tracks),
        menu: Some(Subject::Blend(id.clone())),
    };
    let within = Within::Blend {
        id: id.clone(),
        keys: blend.entries().iter().map(|entry| entry.key()).collect(),
    };
    let blocks = match tracks.is_empty() {
        true => vec![
            Block::Header(Box::new(header)),
            Block::Notice(MaterialSymbol::QUEUE_MUSIC, tr!("music.blend.empty")),
        ],
        false => listed(header, Leading::Cover, Some(within)),
    };
    page_of(blocks, listening)
}

fn saved_in(mirrors: &[music_blend::Mirror]) -> String {
    let mut names: Vec<&str> = Vec::new();
    for mirror in mirrors {
        if !names.contains(&mirror.service.name) {
            names.push(mirror.service.name);
        }
    }
    match names.is_empty() {
        true => tr!("music.blend.local").to_string(),
        false => tr!("music.blend.saved-in", names = names.join(" · ").as_str()).to_string(),
    }
}

fn album(source: Source, id: Id, listening: Listening) -> Element {
    let catalog = source.catalog;
    let album_id = id.clone();
    let loading = fetch(("music-album", source.account, id), move || async move {
        catalog.album(&album_id).await
    });
    fetched(loading, move |page: &AlbumPage| {
        let header = Header {
            artwork: page.album.artwork.clone(),
            round: false,
            kind: tr!("music.kind.album"),
            title: page.album.name.clone(),
            lines: vec![
                page.album.byline(),
                format!("{} · {}", release(&page.album), songs(page.tracks.len())),
            ],
            fidelity: page.album.fidelity,
            tracks: Arc::clone(&page.tracks),
            menu: None,
        };
        page_of(listed(header, Leading::Number, None), listening.clone())
    })
}

fn artist(source: Source, id: Id, listening: Listening) -> Element {
    let catalog = source.catalog;
    let artist_id = id.clone();
    let loading = fetch(("music-artist", source.account, id), move || async move {
        catalog.artist(&artist_id).await
    });
    fetched(loading, move |page: &ArtistPage| {
        let mut blocks = vec![Block::Header(Box::new(Header {
            artwork: page.artist.artwork.clone(),
            round: true,
            kind: tr!("music.kind.artist"),
            title: page.artist.name.clone(),
            lines: Vec::new(),
            fidelity: None,
            tracks: Arc::clone(&page.popular),
            menu: None,
        }))];
        if !page.popular.is_empty() {
            blocks.push(Block::Heading(tr!("music.artist.popular")));
            let popular: Arc<[Track]> = page.popular.iter().take(POPULAR).cloned().collect();
            blocks.push(Block::tracks(&popular, Leading::Cover));
        }
        if !page.albums.is_empty() {
            let albums = Cards::Albums(Arc::clone(&page.albums));
            shelved_cards(&mut blocks, tr!("music.artist.albums"), albums);
        }
        if !page.singles.is_empty() {
            let singles = Cards::Albums(Arc::clone(&page.singles));
            shelved_cards(&mut blocks, tr!("music.artist.singles"), singles);
        }
        page_of(blocks, listening.clone())
    })
}

fn mix(source: Source, seed: Track, listening: Listening) -> Element {
    let catalog = source.catalog;
    let mix_seed = seed.clone();
    let loading = fetch(
        ("music-mix", source.account, seed.id.clone()),
        move || async move { catalog.mix(&mix_seed).await },
    );
    fetched(loading, move |mixed: &Arc<[Track]>| {
        let header = Header {
            artwork: seed.artwork.clone(),
            round: false,
            kind: tr!("music.kind.mix"),
            title: tr!("music.mix.title", name = seed.byline().as_str()).to_string(),
            lines: vec![
                tr!("music.mix.seeded", name = seed.name.as_str()).to_string(),
                songs(mixed.len()).to_string(),
            ],
            fidelity: None,
            tracks: Arc::clone(mixed),
            menu: None,
        };
        page_of(listed(header, Leading::Cover, None), listening.clone())
    })
}

fn songs_of(title: Label, tracks: &Arc<[Track]>, listening: Listening) -> Element {
    let header = Header {
        artwork: tracks
            .first()
            .map(|track| track.artwork.clone())
            .unwrap_or_default(),
        round: false,
        kind: tr!("music.kind.collection"),
        title: title.to_string(),
        lines: vec![songs(tracks.len()).to_string()],
        fidelity: None,
        tracks: Arc::clone(tracks),
        menu: None,
    };
    page_of(listed(header, Leading::Cover, None), listening)
}

fn cards_of(title: Label, cards: Cards) -> Element {
    let count = cards.len();
    column! {
        container(text(title).display().bold())
            .padding(Inset::edges(Space::WIDE, Space::ROOMY, Space::SNUG, Space::ROOMY))
            .fill_width();
        lazy_tiles(Length::px(CARD), count, move |slot| cards.card(slot.index))
            .gap(Space::SNUG)
            .padding(Inset::edges(Space::NONE, Space::LOOSE, Space::WIDE, Space::LOOSE))
            .grow(1.0)
            .fill_width();
    }
    .fill()
    .any()
}
