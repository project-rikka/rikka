use rikka_appearance::Appearance;
use rikka_settings::session::appearance::Background;
use rikka_theme::Theme;
use rustc_hash::FxHashMap;

use crate::embedded::SurfaceId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SurfaceBackground {
    #[default]
    Embedding,
    Desktop,
    Fixed(Background),
}

struct Dressed {
    background: Option<Background>,
    worn: Theme,
}

pub(crate) struct Look {
    session: Appearance,
    following: Theme,
    theme: Option<String>,
    background: Option<Background>,
    worn: Theme,
    surfaces: FxHashMap<SurfaceId, Dressed>,
}

impl Look {
    pub(crate) fn new(session: Appearance) -> Self {
        let following = session.theme();
        Self {
            session,
            worn: following.clone(),
            following,
            theme: None,
            background: None,
            surfaces: FxHashMap::default(),
        }
    }

    pub(crate) fn worn(&self) -> &Theme {
        &self.worn
    }

    pub(crate) fn worn_by(&self, id: SurfaceId) -> &Theme {
        self.surfaces
            .get(&id)
            .map_or(&self.worn, |dressed| &dressed.worn)
    }

    pub(crate) fn follow(&mut self, session: Appearance, theme: Option<Theme>) -> bool {
        self.session = session;
        let overridden =
            self.theme.is_some() || self.background.is_some() || !self.surfaces.is_empty();
        match theme {
            Some(theme) => self.following = theme,
            None if !overridden => return false,
            None => {}
        }
        self.wear()
    }

    pub(crate) fn set_theme(&mut self, slug: Option<String>) -> bool {
        if self.theme == slug {
            return false;
        }
        self.theme = slug;
        self.wear()
    }

    pub(crate) fn set_background(&mut self, background: Option<Background>) -> bool {
        if self.background == background {
            return false;
        }
        self.background = background;
        self.wear()
    }

    pub(crate) fn set_surface_background(
        &mut self,
        id: SurfaceId,
        background: SurfaceBackground,
    ) -> bool {
        let background = match background {
            SurfaceBackground::Embedding => {
                return self.surfaces.remove(&id).is_some();
            }
            SurfaceBackground::Desktop => None,
            SurfaceBackground::Fixed(background) => Some(background),
        };
        if self
            .surfaces
            .get(&id)
            .is_some_and(|dressed| dressed.background == background)
        {
            return false;
        }
        let worn = self.dress(background);
        self.surfaces.insert(id, Dressed { background, worn });
        true
    }

    pub(crate) fn forget(&mut self, id: SurfaceId) {
        self.surfaces.remove(&id);
    }

    fn wear(&mut self) -> bool {
        let theme = self.dress(self.background);
        let mut changed = theme != self.worn;
        self.worn = theme;
        let backgrounds: Vec<(SurfaceId, Option<Background>)> = self
            .surfaces
            .iter()
            .map(|(id, dressed)| (*id, dressed.background))
            .collect();
        for (id, background) in backgrounds {
            let worn = self.dress(background);
            if let Some(dressed) = self.surfaces.get_mut(&id)
                && dressed.worn != worn
            {
                dressed.worn = worn;
                changed = true;
            }
        }
        changed
    }

    fn dress(&self, background: Option<Background>) -> Theme {
        match (&self.theme, background) {
            (None, None) => self.following.clone(),
            (slug, background) => {
                let mut appearance = self.session.clone();
                if let Some(slug) = slug {
                    appearance = appearance.with_theme(slug);
                }
                if let Some(background) = background {
                    appearance = appearance.with_background(background);
                }
                appearance.theme()
            }
        }
    }
}
