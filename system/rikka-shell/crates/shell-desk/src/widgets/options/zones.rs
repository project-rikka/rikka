use std::convert::Infallible;
use std::rc::Rc;
use std::sync::Arc;

use jiff::{Timestamp, tz};
use rikka_ui::{
    Element, Fetch, Label, Length, MaterialSymbol, Space, Styled, TextDocument, fetch_blocking,
    local_state, tr,
};
use rikka_widgets::{column, list_item, list_of, text_field};

use crate::widgets::clock::{Offset, city, offset};

const LISTED: &str = "desk.zones";
const ROOM: f32 = 248.0;
const REGIONS: [&str; 9] = [
    "Africa/",
    "America/",
    "Antarctica/",
    "Asia/",
    "Atlantic/",
    "Australia/",
    "Europe/",
    "Indian/",
    "Pacific/",
];

#[derive(Debug)]
struct Place {
    name: String,
    city: String,
    sought: String,
    offset: i32,
}

#[derive(Debug, Default)]
struct Listing {
    here: Option<String>,
    offset: i32,
    places: Vec<Place>,
}

pub(crate) type Pick = Rc<dyn Fn(Option<String>)>;

#[must_use]
pub(super) fn zones(current: Option<String>, pick: Pick) -> Element {
    local_state(TextDocument::new(""), move |query: TextDocument, _| {
        let typed = query.text().trim().to_lowercase();
        let listing = match fetch_blocking(LISTED, || Ok::<_, Infallible>(listing())) {
            Fetch::Ready(listing) => listing,
            Fetch::Loading | Fetch::Failed(_) => Arc::default(),
        };
        let rows: Rc<Vec<Option<usize>>> = Rc::new(
            typed
                .is_empty()
                .then_some(None)
                .into_iter()
                .chain(
                    listing
                        .places
                        .iter()
                        .enumerate()
                        .filter(|(_, place)| place.sought.contains(&typed))
                        .map(|(index, _)| Some(index)),
                )
                .collect(),
        );
        let selected = rows.iter().position(|row| {
            row.map(|index| listing.places[index].name.as_str()) == current.as_deref()
        });
        let (listed, chosen, named) = (Rc::clone(&rows), Rc::clone(&rows), Arc::clone(&listing));
        let pick = Rc::clone(&pick);
        column! {
            text_field(&query).placeholder(tr!("shell.desk.widget.options.zone-search"));
            list_of(listed.len(), move |index| match listed[index].map(|at| &listing.places[at]) {
                None => list_item(tr!("shell.desk.widget.clock.local"))
                    .icon(MaterialSymbol::HOME)
                    .secondary(match &listing.here {
                        Some(name) => Label::from(name.as_str()),
                        None => Label::from(utc(listing.offset)),
                    })
                    .trailing(Label::from(utc(listing.offset))),
                Some(place) => list_item(Label::from(place.city.as_str()))
                    .secondary(Label::from(place.name.as_str()))
                    .trailing(Label::from(utc(place.offset))),
            })
            .selected(selected)
            .on_select(move |index: usize| {
                if let Some(row) = chosen.get(index) {
                    pick(row.map(|at| named.places[at].name.clone()));
                }
            })
            .height(Length::px(ROOM))
            .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width()
    })
}

fn listing() -> Listing {
    let now = Timestamp::now();
    let database = tz::db();
    let mut places: Vec<Place> = database
        .available()
        .filter_map(|name| {
            let name = name.as_str().to_owned();
            if !REGIONS.iter().any(|region| name.starts_with(region)) {
                return None;
            }
            let zone = database.get(&name).ok()?;
            let city = city(&name);
            Some(Place {
                sought: format!("{} {}", city.to_lowercase(), name.to_lowercase()),
                offset: zone.to_offset(now).seconds(),
                city,
                name,
            })
        })
        .collect();
    places.sort_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then(left.city.cmp(&right.city))
    });
    let here = tz::TimeZone::system();
    Listing {
        here: here.iana_name().map(str::to_owned),
        offset: here.to_offset(now).seconds(),
        places,
    }
}

fn utc(seconds: i32) -> String {
    let Offset {
        sign,
        hours,
        minutes,
    } = offset(seconds);
    match minutes {
        0 => format!("UTC{sign}{hours}"),
        _ => format!("UTC{sign}{hours}:{minutes:02}"),
    }
}
