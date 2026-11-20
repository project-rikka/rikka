use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use rikka_i18n_catalog::{Args, Catalog, Locale, Message};

use crate::builder::I18nBuilder;

#[derive(Clone, Debug)]
pub struct I18n {
    catalogs: Arc<BTreeMap<Locale, Catalog>>,
    fallbacks: Arc<[Locale]>,
    locale: Locale,
    chain: Vec<Locale>,
}

impl I18n {
    pub fn builder() -> I18nBuilder {
        I18nBuilder::default()
    }

    pub fn empty() -> Self {
        Self::new(BTreeMap::new(), Vec::new(), Locale::und())
    }

    pub(crate) fn new(
        catalogs: BTreeMap<Locale, Catalog>,
        fallbacks: Vec<Locale>,
        locale: Locale,
    ) -> Self {
        Self::from_parts(Arc::new(catalogs), fallbacks.into(), locale)
    }

    fn from_parts(
        catalogs: Arc<BTreeMap<Locale, Catalog>>,
        fallbacks: Arc<[Locale]>,
        locale: Locale,
    ) -> Self {
        let chain = resolve_chain(&catalogs, &fallbacks, &locale);
        Self {
            catalogs,
            fallbacks,
            locale,
            chain,
        }
    }

    pub fn install(self) {
        crate::install(self);
    }

    pub fn locale(&self) -> &Locale {
        &self.locale
    }

    pub fn with_locale(&self, locale: Locale) -> Self {
        Self::from_parts(
            Arc::clone(&self.catalogs),
            Arc::clone(&self.fallbacks),
            locale,
        )
    }

    pub fn chain(&self) -> &[Locale] {
        &self.chain
    }

    pub fn catalogs(&self) -> impl Iterator<Item = &Catalog> {
        self.catalogs.values()
    }

    pub fn negotiate<'a>(&self, requested: impl IntoIterator<Item = &'a Locale>) -> Option<Locale> {
        let available: Vec<Locale> = self.catalogs.keys().cloned().collect();
        rikka_i18n_catalog::negotiate(requested, &available)
    }

    pub fn message(&self, key: &str) -> Option<&Message> {
        self.lookup(key).map(|(_, message)| message)
    }

    fn lookup(&self, key: &str) -> Option<(&Locale, &Message)> {
        self.chain
            .iter()
            .find_map(|locale| Some((locale, self.catalogs.get(locale)?.get(key)?)))
    }

    pub fn contains(&self, key: &str) -> bool {
        self.message(key).is_some()
    }

    pub fn translate(&self, key: &str) -> Option<Arc<str>> {
        self.translate_with(key, Args::EMPTY)
    }

    pub fn translate_with(&self, key: &str, args: &Args<'_>) -> Option<Arc<str>> {
        let (locale, message) = self.lookup(key)?;
        Some(match message.as_literal() {
            Some(literal) => literal.clone(),
            None => message.format(locale.language(), args),
        })
    }

    pub fn tr(&self, key: &str) -> Arc<str> {
        self.translate(key).unwrap_or_else(|| Arc::from(key))
    }

    pub fn tr_with(&self, key: &str, args: &Args<'_>) -> Arc<str> {
        self.translate_with(key, args)
            .unwrap_or_else(|| Arc::from(key))
    }
}

fn resolve_chain(
    catalogs: &BTreeMap<Locale, Catalog>,
    fallbacks: &[Locale],
    locale: &Locale,
) -> Vec<Locale> {
    let mut queue: VecDeque<Locale> = VecDeque::with_capacity(1 + fallbacks.len());
    queue.push_back(locale.clone());
    queue.extend(fallbacks.iter().cloned());

    let mut chain: Vec<Locale> = Vec::new();
    while let Some(requested) = queue.pop_front() {
        for candidate in requested.ancestry() {
            let Some(catalog) = catalogs.get(&candidate) else {
                continue;
            };
            if chain.contains(&candidate) {
                continue;
            }
            if let Some(fallback) = catalog.fallback() {
                queue.push_back(fallback.clone());
            }
            chain.push(candidate);
        }
    }
    chain
}

impl Default for I18n {
    fn default() -> Self {
        Self::empty()
    }
}
