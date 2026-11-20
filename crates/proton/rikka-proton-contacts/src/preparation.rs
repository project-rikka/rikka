use rikka_proton_pgp::random_uuid;

use crate::property::Property;
use crate::vcard::VCard;

const FN: &str = "FN";
const UID: &str = "UID";
const EMAIL: &str = "EMAIL";
const CATEGORIES: &str = "CATEGORIES";
const ADR: &str = "ADR";
const N: &str = "N";
const FALLBACK_NAME: &str = "Unknown";
const UID_PREFIX: &str = "proton-web-";
pub(crate) const PREF_PROPERTIES: [&str; 6] = ["FN", "EMAIL", "TEL", "ADR", "KEY", "PHOTO"];

#[must_use]
pub fn prepare_for_saving(card: &VCard) -> VCard {
    let mut prepared = VCard::new();

    for property in card.to_v4().properties() {
        if is_empty(property) {
            continue;
        }
        if property.is(CATEGORIES) {
            for category in property.list_value() {
                let category = category.trim();
                if category.is_empty() {
                    continue;
                }
                let mut single = property.clone();
                single.set_text_value(category);
                prepared.push(single);
            }
            continue;
        }
        prepared.push(property.clone());
    }

    for name in PREF_PROPERTIES {
        renumber_pref(&mut prepared, name);
    }
    group_emails(&mut prepared);
    ensure_identity(&mut prepared);

    prepared
}

fn is_empty(property: &Property) -> bool {
    if property.is(N) {
        return false;
    }
    if property.is(ADR) {
        return property
            .components()
            .iter()
            .flatten()
            .all(|part| part.trim().is_empty());
    }
    property.raw_value().trim().is_empty()
}

fn renumber_pref(card: &mut VCard, name: &str) {
    let mut positions: Vec<usize> = card
        .properties()
        .iter()
        .enumerate()
        .filter(|(_, property)| property.is(name))
        .map(|(index, _)| index)
        .collect();
    if positions.is_empty() {
        return;
    }

    let mut ordered: Vec<Property> = positions
        .iter()
        .map(|index| card.properties()[*index].clone())
        .collect();
    ordered.sort_by_key(|property| property.pref().unwrap_or(u32::MAX));
    for (rank, property) in ordered.iter_mut().enumerate() {
        property.set_pref(u32::try_from(rank + 1).unwrap_or(u32::MAX));
    }

    positions.sort_unstable();
    let properties = card.properties_mut();
    for (index, property) in positions.into_iter().zip(ordered) {
        properties[index] = property;
    }
}

fn group_emails(card: &mut VCard) {
    let ungrouped: Vec<usize> = card
        .properties()
        .iter()
        .enumerate()
        .filter(|(_, property)| property.is(EMAIL) && property.group().is_none())
        .map(|(index, _)| index)
        .collect();

    for index in ungrouped {
        let group = card.unused_group();
        card.properties_mut()[index].set_group(Some(group));
    }
}

fn ensure_identity(card: &mut VCard) {
    if card.formatted_name().is_none() {
        card.remove_named(FN);
        let fallback = card
            .structured_name()
            .map(|name| name.display_name())
            .filter(|name| !name.is_empty())
            .or_else(|| card.email_addresses().into_iter().next())
            .unwrap_or_else(|| FALLBACK_NAME.to_owned());
        card.push(Property::text(FN, &fallback).with_pref(1));
    }
    if card.uid().is_none() {
        card.remove_named(UID);
        card.push(Property::text(UID, &generate_uid()));
    }
}

#[must_use]
pub fn generate_uid() -> String {
    format!("{UID_PREFIX}{}", random_uuid())
}
