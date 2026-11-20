use std::collections::HashMap;

use crate::contact::Contact;
use crate::identifier::ContactId;
use crate::preparation::{PREF_PROPERTIES, prepare_for_saving};
use crate::property::Property;
use crate::vcard::{VCard, free_group_name};

const SINGLE_VALUED: [&str; 7] = ["N", "BDAY", "ANNIVERSARY", "GENDER", "PRODID", "REV", "UID"];
const PLACEHOLDER_NAMES: [&str; 2] = ["unknown", "<unknown>"];
const FN: &str = "FN";
const EMAIL: &str = "EMAIL";

struct EmailGroup {
    address: String,
    group: String,
}

pub(crate) fn combine_card_parts(cards: Vec<VCard>) -> VCard {
    let mut combined = VCard::new();
    for card in cards {
        for property in card.properties() {
            if is_single_valued(property) {
                combined.remove_named(property.name());
            } else if combined
                .properties()
                .iter()
                .any(|present| present == property)
            {
                continue;
            }
            combined.push(property.clone());
        }
    }
    combined
}

#[must_use]
pub fn merge_vcards(cards: &[VCard]) -> VCard {
    let prepared: Vec<VCard> = cards.iter().map(prepare_for_saving).collect();
    let Some((first, rest)) = prepared.split_first() else {
        return VCard::new();
    };

    let mut merged = first.clone();
    let mut email_groups: Vec<EmailGroup> = first
        .emails()
        .filter_map(|email| {
            Some(EmailGroup {
                address: email.text_value(),
                group: email.group()?.to_owned(),
            })
        })
        .collect();

    for card in rest {
        let renamed = renamed_groups(card, &mut email_groups);
        for property in card.properties() {
            let mut property = property.clone();
            if let Some(group) = property.group() {
                let replacement = renamed
                    .get(&group.to_ascii_lowercase())
                    .cloned()
                    .unwrap_or_else(|| group.to_owned());
                property.set_group(Some(replacement));
            }

            let present: Vec<&Property> = merged.properties_named(property.name()).collect();
            if present.is_empty() {
                merged.push(property);
                continue;
            }
            let repeated = present
                .iter()
                .any(|present_property| same_value(present_property, &property));
            let addable = !property.is(FN)
                && (property.name().starts_with("X-") || !is_single_valued(&property));
            if repeated || !addable {
                continue;
            }
            if PREF_PROPERTIES.iter().any(|name| property.is(name)) {
                let highest = present
                    .iter()
                    .filter_map(|present_property| present_property.pref())
                    .max()
                    .unwrap_or(0);
                property.set_pref(highest + 1);
            }
            merged.push(property);
        }
    }

    prepare_for_saving(&merged)
}

fn is_single_valued(property: &Property) -> bool {
    SINGLE_VALUED.iter().any(|name| property.is(name))
}

fn renamed_groups(card: &VCard, email_groups: &mut Vec<EmailGroup>) -> HashMap<String, String> {
    let mut renamed = HashMap::new();
    for email in card.emails() {
        let Some(group) = email.group() else {
            continue;
        };
        let address = email.text_value();
        let is_taken = |name: &str| {
            email_groups
                .iter()
                .any(|entry| entry.group.eq_ignore_ascii_case(name))
        };
        let target = if is_taken(group) {
            email_groups
                .iter()
                .find(|entry| entry.address == address)
                .map(|entry| entry.group.clone())
                .unwrap_or_else(|| free_group_name(is_taken))
        } else {
            group.to_owned()
        };
        if !email_groups.iter().any(|entry| entry.address == address) {
            email_groups.push(EmailGroup {
                address,
                group: target.clone(),
            });
        }
        renamed.insert(group.to_ascii_lowercase(), target);
    }
    renamed
}

fn same_value(current: &Property, candidate: &Property) -> bool {
    if candidate.is(EMAIL) {
        return current.text_value().trim() == candidate.text_value().trim();
    }
    let left = current.components();
    let right = candidate.components();
    left.len() == right.len()
        && left
            .iter()
            .zip(&right)
            .all(|(left, right)| normalized(&left.join(",")) == normalized(&right.join(",")))
}

fn normalized(text: &str) -> String {
    text.trim().to_lowercase()
}

#[must_use]
pub fn find_duplicates(contacts: &[Contact]) -> Vec<Vec<ContactId>> {
    let mut parent: Vec<usize> = (0..contacts.len()).collect();
    let mut first_by_key: HashMap<String, usize> = HashMap::new();

    for (index, contact) in contacts.iter().enumerate() {
        let name = normalized(contact.name());
        let mut keys = Vec::new();
        if !name.is_empty() && !PLACEHOLDER_NAMES.contains(&name.as_str()) {
            keys.push(format!("name:{name}"));
        }
        for email in contact.emails() {
            let address = normalized(email.email());
            if !address.is_empty() {
                keys.push(format!("email:{address}"));
            }
        }
        for key in keys {
            match first_by_key.get(&key) {
                Some(other) => union(&mut parent, *other, index),
                None => {
                    first_by_key.insert(key, index);
                }
            }
        }
    }

    let mut groups: Vec<(usize, Vec<ContactId>)> = Vec::new();
    for (index, contact) in contacts.iter().enumerate() {
        let root = find_root(&mut parent, index);
        match groups
            .iter_mut()
            .find(|(group_root, _)| *group_root == root)
        {
            Some((_, members)) => members.push(contact.id().clone()),
            None => groups.push((root, vec![contact.id().clone()])),
        }
    }

    groups
        .into_iter()
        .map(|(_, members)| members)
        .filter(|members| members.len() > 1)
        .collect()
}

fn find_root(parent: &mut [usize], index: usize) -> usize {
    let mut root = index;
    while parent[root] != root {
        root = parent[root];
    }
    let mut node = index;
    while parent[node] != root {
        let next = parent[node];
        parent[node] = root;
        node = next;
    }
    root
}

fn union(parent: &mut [usize], left: usize, right: usize) {
    let left = find_root(parent, left);
    let right = find_root(parent, right);
    if left != right {
        parent[right.max(left)] = right.min(left);
    }
}
