use std::collections::BTreeSet;
use std::path::Path;

use crate::catalog::{Catalog, locale_of};
use crate::error::Error;

const REFERENCE: &str = "en";

pub fn verify_catalogs(sources: &[(&str, &str)]) -> Result<(), Vec<Error>> {
    let mut errors = Vec::new();
    let mut parsed = Vec::new();
    for &(name, source) in sources {
        match Catalog::parse(name, source) {
            Ok(catalog) => {
                if locale_of(name).as_ref() != Some(catalog.locale()) {
                    errors.push(Error::MisnamedLocale {
                        source_name: name.to_owned(),
                        locale: catalog.locale().clone(),
                    });
                }
                parsed.push((name, catalog));
            }
            Err(error) => errors.push(error),
        }
    }

    match parsed.iter().find(|(name, _)| is_reference(name)) {
        Some((reference_name, reference)) => {
            errors.extend(disagreements(reference_name, reference, &parsed));
        }
        None if !sources.iter().any(|(name, _)| is_reference(name)) => {
            errors.push(Error::NoReference);
        }
        None => {}
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn disagreements(
    reference_name: &str,
    reference: &Catalog,
    catalogs: &[(&str, Catalog)],
) -> Vec<Error> {
    if reference.is_empty() {
        return vec![Error::EmptyReference {
            source_name: reference_name.to_owned(),
        }];
    }
    let expected: BTreeSet<&str> = reference.keys().collect();
    catalogs
        .iter()
        .filter(|(name, _)| *name != reference_name)
        .filter_map(|(name, catalog)| {
            let found: BTreeSet<&str> = catalog.keys().collect();
            let missing: Vec<String> = expected
                .difference(&found)
                .map(|&key| key.to_owned())
                .collect();
            let unexpected: Vec<String> = found
                .difference(&expected)
                .map(|&key| key.to_owned())
                .collect();
            (!missing.is_empty() || !unexpected.is_empty()).then(|| Error::Disagrees {
                source_name: (*name).to_owned(),
                reference: reference_name.to_owned(),
                missing,
                unexpected,
            })
        })
        .collect()
}

fn is_reference(name: &str) -> bool {
    Path::new(name)
        .file_stem()
        .is_some_and(|stem| stem == REFERENCE)
}
