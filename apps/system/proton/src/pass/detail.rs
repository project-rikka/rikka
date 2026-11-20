use jiff::Timestamp;
use proton_vaults::{Field, IdentityField, IdentitySection, ItemType, OpenedItem, Section, Value};
use rikka_app::prelude::*;
use rikka_proton_pass::ExtraField;
use rikka_widgets::{column, row};

use crate::pass::rows::{self, Row};
use crate::pass::{Pass, Popup, list};
use crate::{pane, wording};

pub(crate) fn view(vault: &Pass, opened: &OpenedItem) -> impl View {
    let tint = list::tint(opened.item_type);

    column! {
        actions(opened);
        rule();
        scrollable(
            column! {
                heading(vault, opened, tint);
                column_of(sections(vault, opened, tint))
                    .gap(Space::NORMAL)
                    .fill_width();
                facts(vault, opened);
            }
            .gap(Space::NORMAL)
            .padding(Space::ROOMY)
            .fill_width(),
        )
        .grow(1.0)
        .fill_width();
    }
    .fill()
}

const MENU: &str = "proton-pass-item";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Deed {
    Pin,
    Watch,
    History,
    Trash,
}

impl Deed {
    const ALL: [Deed; 4] = [Deed::Pin, Deed::Watch, Deed::History, Deed::Trash];

    fn item(self, opened: &OpenedItem, watched: bool) -> MenuItem {
        match self {
            Deed::Pin => menu_item(match opened.pinned {
                true => tr!("proton.pass.unpin"),
                false => tr!("proton.pass.pin"),
            })
            .icon(MaterialSymbol::PUSH_PIN),
            Deed::Watch => menu_item(match watched {
                true => tr!("proton.pass.exclude"),
                false => tr!("proton.pass.include"),
            })
            .icon(MaterialSymbol::SHIELD),
            Deed::History => menu_item(tr!("proton.pass.history")).icon(MaterialSymbol::HISTORY),
            Deed::Trash => menu_item(tr!("proton.pass.throw")).icon(MaterialSymbol::DELETE),
        }
    }
}

fn actions(opened: &OpenedItem) -> impl View {
    pane::toolbar(
        row! {
            if opened.trashed {
                button(pane::labelled(MaterialSymbol::RESTORE, tr!("proton.pass.restore")))
                .small()
                .on_press(|vault: &mut Pass| vault.restore());
                button(pane::labelled(MaterialSymbol::DELETE_FOREVER, tr!("proton.pass.erase")))
                .danger()
                .small()
                .on_press(|vault: &mut Pass| vault.erase());
            } else {
                button(pane::labelled(MaterialSymbol::EDIT, tr!("proton.pass.edit")))
                .small()
                .on_press(|vault: &mut Pass| vault.edit());
                button(pane::labelled(MaterialSymbol::HISTORY, tr!("proton.pass.history")))
                .ghost()
                .small()
                .on_press(|vault: &mut Pass| vault.show_history());
            }
            spacer().grow(1.0);
            if !opened.trashed {
                button(icon(MaterialSymbol::MORE_VERT).extent(Length::px(15.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(|vault: &mut Pass| vault.popped(Some(Popup::Deeds)))
                    .build()
                    .keyed(MENU);
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
}

pub(crate) fn deeds(vault: &Pass, opened: &OpenedItem) -> Element {
    let watched = !vault
        .entry_of(&opened.share, &opened.id)
        .is_some_and(|entry| entry.is_excluded());

    menu()
        .items(Deed::ALL.map(|deed| {
            deed.item(opened, watched)
                .on_press(move |vault: &mut Pass| vault.did(deed))
        }))
        .on_dismiss(|vault: &mut Pass| vault.popped(None))
        .build()
        .anchored(Anchor::below(MENU).offset(0.0, 4.0))
}

fn heading(vault: &Pass, opened: &OpenedItem, tint: Color) -> impl View {
    let vault_name = vault
        .vaults()
        .vault(&opened.share)
        .map(|vault| vault.name.clone());

    row! {
        pane::avatar(&wording::initial(&opened.title), tint);
        column! {
            row! {
                text(opened.title.clone()).title().semibold().grow(1.0);
                if opened.pinned {
                    icon(MaterialSymbol::PUSH_PIN)
                        .extent(Length::px(14.0))
                        .color(palette().text_muted);
                }
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .fill_width();
            row! {
                icon(list::symbol(opened.item_type))
                    .extent(Length::px(13.0))
                    .color(palette().text_muted);
                text(list::named(opened.item_type)).caption().muted().no_wrap();
                if let Some(name) = vault_name {
                    text("·").caption().muted();
                    icon(MaterialSymbol::SHIELD_LOCK)
                        .extent(Length::px(13.0))
                        .color(palette().text_muted);
                    text(name).caption().muted().no_wrap();
                }
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center);
        }
        .gap(Space::TIGHT)
        .grow(1.0);
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width()
}

fn sections(vault: &Pass, opened: &OpenedItem, tint: Color) -> Vec<Element> {
    let mut sections: Vec<Element> = Vec::new();
    sections.extend(main(vault, opened, tint));
    sections.extend(websites(opened));
    sections.extend(note(opened));
    sections.extend(identity(vault, opened, tint));
    sections.extend(fields(vault, opened, tint));
    sections
}

fn main(vault: &Pass, opened: &OpenedItem, tint: Color) -> Option<Element> {
    let key = |name: &str| key(opened, name);
    let mut rows: Vec<Row> = Vec::new();

    match opened.item_type {
        ItemType::Login => {
            if !opened.email.is_empty() {
                rows.push(rows::field(
                    &key("email"),
                    MaterialSymbol::MAIL,
                    tr!("proton.pass.email"),
                    &opened.email,
                ));
            }
            if !opened.username.is_empty() {
                rows.push(rows::field(
                    &key("username"),
                    MaterialSymbol::PERSON,
                    tr!("proton.pass.username"),
                    &opened.username,
                ));
            }
            if !opened.password.is_empty() {
                let mut row = rows::field(
                    &key("password"),
                    MaterialSymbol::KEY,
                    tr!("proton.pass.password"),
                    &opened.password,
                )
                .hidden(vault);
                if let Some(strength) = opened.strength {
                    row = row.below(rows::strength(strength));
                }
                rows.push(row);
            }
        }
        ItemType::Alias => {
            if !opened.alias.is_empty() {
                rows.push(rows::field(
                    &key("alias"),
                    MaterialSymbol::ALTERNATE_EMAIL,
                    tr!("proton.pass.alias"),
                    &opened.alias,
                ));
            }
        }
        ItemType::Card => {
            let card = &opened.card;
            if !card.holder.is_empty() {
                rows.push(rows::field(
                    &key("holder"),
                    MaterialSymbol::PERSON,
                    tr!("proton.pass.card-holder"),
                    &card.holder,
                ));
            }
            if !card.number.is_empty() {
                let shown = vault.is_revealed(&key("number"));
                rows.push(
                    rows::field(
                        &key("number"),
                        MaterialSymbol::CREDIT_CARD,
                        tr!("proton.pass.card-number"),
                        &card.number,
                    )
                    .hidden(vault)
                    .masked(rows::card_number(&card.number, shown)),
                );
            }
            if !card.expiration.is_empty() {
                rows.push(
                    rows::field(
                        &key("expiration"),
                        MaterialSymbol::CALENDAR_MONTH,
                        tr!("proton.pass.card-expiration"),
                        &card.expiration,
                    )
                    .masked(rows::expiration(&card.expiration)),
                );
            }
            if !card.verification.is_empty() {
                rows.push(
                    rows::field(
                        &key("verification"),
                        MaterialSymbol::VERIFIED_USER,
                        tr!("proton.pass.card-verification"),
                        &card.verification,
                    )
                    .hidden(vault),
                );
            }
            if !card.pin.is_empty() {
                rows.push(
                    rows::field(
                        &key("pin"),
                        MaterialSymbol::PASSWORD,
                        tr!("proton.pass.card-pin"),
                        &card.pin,
                    )
                    .hidden(vault),
                );
            }
        }
        ItemType::Wifi => {
            let wifi = &opened.wifi;
            if !wifi.ssid.is_empty() {
                rows.push(rows::field(
                    &key("ssid"),
                    MaterialSymbol::WIFI,
                    tr!("proton.pass.wifi-name"),
                    &wifi.ssid,
                ));
            }
            if !wifi.password.is_empty() {
                rows.push(
                    rows::field(
                        &key("wifi-password"),
                        MaterialSymbol::KEY,
                        tr!("proton.pass.password"),
                        &wifi.password,
                    )
                    .hidden(vault),
                );
            }
            rows.push(
                rows::field(
                    &key("security"),
                    MaterialSymbol::SHIELD,
                    tr!("proton.pass.wifi-security"),
                    list::secured(wifi.security).as_str(),
                )
                .plain(),
            );
        }
        ItemType::SshKey => {
            if !opened.ssh.public.is_empty() {
                rows.push(rows::field(
                    &key("public"),
                    MaterialSymbol::VPN_KEY,
                    tr!("proton.pass.public-key"),
                    &opened.ssh.public,
                ));
            }
            if !opened.ssh.private.is_empty() {
                rows.push(
                    rows::field(
                        &key("private"),
                        MaterialSymbol::KEY,
                        tr!("proton.pass.private-key"),
                        &opened.ssh.private,
                    )
                    .hidden(vault),
                );
            }
        }
        ItemType::Note | ItemType::Identity | ItemType::Custom => {}
    }

    let mut listed: Vec<Element> = rows
        .into_iter()
        .map(|row| Element::from(row.tint(tint)))
        .collect();
    listed.extend(codes(vault, opened));

    rows::section(None, listed)
}

fn codes(vault: &Pass, opened: &OpenedItem) -> Vec<Element> {
    opened
        .codes()
        .map(|(name, uri)| {
            let name = match name.is_empty() {
                true => tr!("proton.pass.code"),
                false => Label::from(name),
            };
            rows::code(name, uri, vault.now())
        })
        .collect()
}

fn websites(opened: &OpenedItem) -> Option<Element> {
    rows::section(
        Some(tr!("proton.pass.websites")),
        opened.urls.iter().map(|url| rows::link(url)).collect(),
    )
}

fn note(opened: &OpenedItem) -> Option<Element> {
    (!opened.note.is_empty()).then(|| rows::note(opened.id.as_str(), &opened.note))
}

fn identity(vault: &Pass, opened: &OpenedItem, tint: Color) -> Vec<Element> {
    let Some(identity) = opened.identity() else {
        return Vec::new();
    };

    IdentitySection::ALL
        .into_iter()
        .filter_map(|facet| {
            let mut rows: Vec<Element> = facet
                .fields()
                .map(|(_, field)| field)
                .filter(|field| !field.read(identity).is_empty())
                .map(|field| {
                    let row = rows::field(
                        &key(opened, &format!("{field:?}")),
                        field_symbol(field),
                        field_label(field),
                        field.read(identity),
                    )
                    .tint(tint);

                    Element::from(match field.is_hidden() {
                        true => row.hidden(vault),
                        false => row,
                    })
                })
                .collect();
            rows.extend(extras(vault, opened, facet.extras(identity), tint));

            rows::section(Some(titled(facet)), rows)
        })
        .collect()
}

pub(crate) fn titled(facet: IdentitySection) -> Label {
    match facet {
        IdentitySection::Personal => tr!("proton.pass.identity.personal"),
        IdentitySection::Address => tr!("proton.pass.identity.address"),
        IdentitySection::Contact => tr!("proton.pass.identity.contact"),
        IdentitySection::Work => tr!("proton.pass.identity.work"),
    }
}

pub(crate) fn field_label(field: IdentityField) -> Label {
    match field {
        IdentityField::FullName => tr!("proton.pass.identity.full-name"),
        IdentityField::FirstName => tr!("proton.pass.identity.first-name"),
        IdentityField::MiddleName => tr!("proton.pass.identity.middle-name"),
        IdentityField::LastName => tr!("proton.pass.identity.last-name"),
        IdentityField::Email => tr!("proton.pass.email"),
        IdentityField::Phone => tr!("proton.pass.identity.phone"),
        IdentityField::Birthdate => tr!("proton.pass.identity.birthdate"),
        IdentityField::Gender => tr!("proton.pass.identity.gender"),
        IdentityField::Organization => tr!("proton.pass.identity.organization"),
        IdentityField::Street => tr!("proton.pass.identity.street"),
        IdentityField::Floor => tr!("proton.pass.identity.floor"),
        IdentityField::City => tr!("proton.pass.identity.city"),
        IdentityField::PostalCode => tr!("proton.pass.identity.postal-code"),
        IdentityField::Province => tr!("proton.pass.identity.province"),
        IdentityField::County => tr!("proton.pass.identity.county"),
        IdentityField::Country => tr!("proton.pass.identity.country"),
        IdentityField::SocialSecurity => tr!("proton.pass.identity.social-security"),
        IdentityField::Passport => tr!("proton.pass.identity.passport"),
        IdentityField::Licence => tr!("proton.pass.identity.licence"),
        IdentityField::Website => tr!("proton.pass.identity.website"),
        IdentityField::SecondPhone => tr!("proton.pass.identity.second-phone"),
        IdentityField::LinkedIn => tr!("proton.pass.identity.linkedin"),
        IdentityField::XHandle => tr!("proton.pass.identity.x-handle"),
        IdentityField::Instagram => tr!("proton.pass.identity.instagram"),
        IdentityField::Facebook => tr!("proton.pass.identity.facebook"),
        IdentityField::Reddit => tr!("proton.pass.identity.reddit"),
        IdentityField::Yahoo => tr!("proton.pass.identity.yahoo"),
        IdentityField::Company => tr!("proton.pass.identity.company"),
        IdentityField::Job => tr!("proton.pass.identity.job"),
        IdentityField::WorkPhone => tr!("proton.pass.identity.work-phone"),
        IdentityField::WorkEmail => tr!("proton.pass.identity.work-email"),
        IdentityField::PersonalWebsite => tr!("proton.pass.identity.personal-website"),
    }
}

fn field_symbol(field: IdentityField) -> MaterialSymbol {
    match field {
        IdentityField::Email | IdentityField::WorkEmail => MaterialSymbol::MAIL,
        IdentityField::Phone | IdentityField::SecondPhone | IdentityField::WorkPhone => {
            MaterialSymbol::CALL
        }
        IdentityField::SocialSecurity | IdentityField::Passport | IdentityField::Licence => {
            MaterialSymbol::BADGE
        }
        IdentityField::Website | IdentityField::PersonalWebsite => MaterialSymbol::LINK,
        _ => MaterialSymbol::CHEVRON_RIGHT,
    }
}

fn fields(vault: &Pass, opened: &OpenedItem, tint: Color) -> Vec<Element> {
    let mut listed = Vec::new();

    listed.extend(rows::section(
        Some(tr!("proton.pass.fields")),
        field_rows(vault, opened, opened.fields.iter().cloned(), tint),
    ));
    for Section { name, fields } in &opened.sections {
        listed.extend(rows::section(
            Some(Label::from(name.as_str())),
            field_rows(vault, opened, fields.iter().cloned(), tint),
        ));
    }

    listed
}

fn facts(vault: &Pass, opened: &OpenedItem) -> impl View {
    let rows: Vec<Element> = vec![
        Element::from(
            rows::field(
                "revision",
                MaterialSymbol::EDIT,
                tr!(
                    "proton.pass.revisions",
                    count = &opened.revision.to_string()
                ),
                &wording::whole(opened.modified),
            )
            .plain(),
        ),
        Element::from(
            rows::field(
                "created",
                MaterialSymbol::BOLT,
                tr!("proton.pass.created"),
                &wording::whole(opened.created),
            )
            .plain(),
        ),
    ];

    column! {
        column_of(rows::section(None, rows)).fill_width();
        row! {
            button(pane::labelled(MaterialSymbol::HISTORY, tr!("proton.pass.history")))
            .ghost()
            .small()
            .on_press(|vault: &mut Pass| vault.show_history());
            spacer().grow(1.0);
            button(pane::labelled(MaterialSymbol::INFO, tr!("proton.pass.more-info")))
            .ghost()
            .small()
            .on_press(|vault: &mut Pass| vault.tell_more());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        if vault.tells_more() {
            card(
                column! {
                    pane::fact(tr!("proton.pass.item-id"), opened.id.as_str().to_owned());
                    pane::fact(tr!("proton.pass.vault-id"), opened.share.as_str().to_owned());
                    if opened.used > Timestamp::UNIX_EPOCH {
                        pane::fact(tr!("proton.pass.last-used"), wording::whole(opened.used));
                    }
                }
                .gap(Space::TIGHT)
                .fill_width(),
            )
            .fill_width();
        }
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn extras(vault: &Pass, opened: &OpenedItem, extras: &[ExtraField], tint: Color) -> Vec<Element> {
    field_rows(vault, opened, extras.iter().map(Field::from_extra), tint)
}

fn field_rows(
    vault: &Pass,
    opened: &OpenedItem,
    fields: impl IntoIterator<Item = Field>,
    tint: Color,
) -> Vec<Element> {
    fields
        .into_iter()
        .map(|field| match &field.value {
            Value::Code(uri) => rows::code(Label::from(field.name.as_str()), uri, vault.now()),
            value => {
                let text = match value {
                    Value::Timestamp(at) => Timestamp::from_second(*at)
                        .map(wording::whole)
                        .unwrap_or_default(),
                    value => value.text(),
                };
                let row = rows::field(
                    &key(opened, &field.name),
                    MaterialSymbol::CHEVRON_RIGHT,
                    Label::from(field.name.as_str()),
                    &text,
                )
                .tint(tint);

                Element::from(match value.is_hidden() {
                    true => row.hidden(vault),
                    false => row,
                })
            }
        })
        .collect()
}

fn key(opened: &OpenedItem, name: &str) -> String {
    format!("{}.{name}", opened.id)
}
