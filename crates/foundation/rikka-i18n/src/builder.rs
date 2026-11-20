use std::collections::BTreeMap;
use std::path::Path;

use rikka_i18n_catalog::{Catalog, Error, IntoLocale, Locale, Result, negotiate};

use crate::i18n::I18n;
use crate::loader;

const FALLBACK: &str = "en";

#[derive(Debug, Default)]
pub struct I18nBuilder {
    catalogs: BTreeMap<Locale, Catalog>,
    fallbacks: Vec<Locale>,
    preferences: Vec<Locale>,
    locale: Option<Locale>,
    error: Option<Error>,
}

impl I18nBuilder {
    pub fn embedded(mut self, catalogs: &[(&str, &str)]) -> Self {
        for (name, source) in catalogs {
            self.add(Catalog::parse(name, source));
        }
        self.fallback(FALLBACK)
    }

    pub fn source(mut self, name: &str, source: &str) -> Self {
        self.add(Catalog::parse(name, source));
        self
    }

    pub fn load_dir(mut self, dir: impl AsRef<Path>) -> Self {
        let report = loader::load_dir(dir);
        report.warn();
        for catalog in report.loaded {
            self.add(Ok(catalog));
        }
        self
    }

    pub fn fallback(mut self, locale: impl IntoLocale) -> Self {
        if let Some(locale) = self.parse_locale(locale) {
            self.fallbacks.push(locale);
        }
        self
    }

    pub fn locale(mut self, locale: impl IntoLocale) -> Self {
        if let Some(locale) = self.parse_locale(locale) {
            self.locale = Some(locale);
        }
        self
    }

    pub fn prefer(mut self, locale: impl IntoLocale) -> Self {
        if let Some(locale) = self.parse_locale(locale) {
            self.preferences.push(locale);
        }
        self
    }

    pub fn build(self) -> Result<I18n> {
        let Self {
            catalogs,
            fallbacks,
            mut preferences,
            locale,
            error,
        } = self;
        if let Some(error) = error {
            return Err(error);
        }

        let locale = locale.unwrap_or_else(|| {
            if preferences.is_empty() {
                preferences.extend(Locale::system());
            }
            let available: Vec<Locale> = catalogs.keys().cloned().collect();
            negotiate(&preferences, &available)
                .or_else(|| fallbacks.first().cloned())
                .unwrap_or_default()
        });
        Ok(I18n::new(catalogs, fallbacks, locale))
    }

    pub fn install(self) -> Result<()> {
        crate::install(self.build()?);
        Ok(())
    }

    fn add(&mut self, catalog: Result<Catalog>) {
        let catalog = match catalog {
            Ok(catalog) => catalog,
            Err(error) => return self.fail(error),
        };
        match self.catalogs.get_mut(catalog.locale()) {
            Some(loaded) => loaded.merge(catalog),
            None => {
                self.catalogs.insert(catalog.locale().clone(), catalog);
            }
        }
    }

    fn parse_locale(&mut self, locale: impl IntoLocale) -> Option<Locale> {
        match locale.into_locale() {
            Ok(locale) => Some(locale),
            Err(error) => {
                self.fail(error);
                None
            }
        }
    }

    fn fail(&mut self, error: Error) {
        if self.error.is_none() {
            self.error = Some(error);
        }
    }
}
