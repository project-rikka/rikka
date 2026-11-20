use rikka_settings::Store;
use rikka_settings::session::appearance::{self, Background};
use rikka_theme::{MaterialYou, Result, Theme, ThemeRegistry, ThemeSpec, Translucency};
use rikka_types::Lens;

use crate::session::read;

#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct Appearance {
    slug: String,
    material: MaterialYou,
    background: Background,
    glaze: Translucency,
}

impl Appearance {
    pub fn read(store: &Store) -> Appearance {
        Appearance {
            slug: store.get::<String>(appearance::THEME).unwrap_or_default(),
            material: read::material(store),
            background: store.choice(appearance::BACKGROUND),
            glaze: read::glaze(store),
        }
    }

    pub fn current() -> Appearance {
        match Store::open(appearance::schema()) {
            Ok(store) => Appearance::read(&store),
            Err(error) => {
                tracing::debug!("failed to read appearance settings: {error}");
                Appearance {
                    slug: String::new(),
                    material: MaterialYou::default(),
                    background: Background::default(),
                    glaze: Translucency::opaque(),
                }
            }
        }
    }

    pub fn update(&mut self, store: &Store) -> Option<Theme> {
        let current = Appearance::read(store);
        let theme_changed = current.changes_theme_of(self);
        *self = current;
        theme_changed.then(|| self.theme())
    }

    pub fn theme(&self) -> Theme {
        self.theme_and_lens().0
    }

    pub fn theme_and_lens(&self) -> (Theme, Option<Lens>) {
        let theme = Theme::from_spec(&self.spec());
        let lens = theme.translucency().declared_lens();
        (theme.with_translucency(self.glaze), lens)
    }

    fn spec(&self) -> ThemeSpec {
        let spec = ThemeRegistry::named(&self.slug, self.material);
        match self.background {
            Background::Solid => spec,
            Background::Glass => spec.glass(),
            Background::Liquid => spec.liquid(),
        }
    }

    pub fn slug(&self) -> &str {
        &self.slug
    }

    pub fn material(&self) -> MaterialYou {
        self.material
    }

    pub fn registry(&self) -> Result<ThemeRegistry> {
        ThemeRegistry::discover(self.material)
    }

    pub fn with_theme(mut self, slug: &str) -> Appearance {
        slug.clone_into(&mut self.slug);
        self
    }

    fn changes_theme_of(&self, previous: &Appearance) -> bool {
        self.slug != previous.slug
            || self.background != previous.background
            || self.glaze != previous.glaze
            || (self.material != previous.material && MaterialYou::names(&self.slug))
    }
}
