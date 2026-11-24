use rikka_appearance::Appearance;
use rikka_settings::session::appearance::Background;
use rikka_theme::Theme;

pub(crate) struct Look {
    session: Appearance,
    following: Theme,
    theme: Option<String>,
    background: Option<Background>,
    worn: Theme,
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
        }
    }

    pub(crate) fn worn(&self) -> &Theme {
        &self.worn
    }

    pub(crate) fn follow(&mut self, session: Appearance, theme: Option<Theme>) -> Option<Theme> {
        self.session = session;
        let overridden = self.theme.is_some() || self.background.is_some();
        match theme {
            Some(theme) => self.following = theme,
            None if !overridden => return None,
            None => {}
        }
        self.wear()
    }

    pub(crate) fn set_theme(&mut self, slug: Option<String>) -> Option<Theme> {
        if self.theme == slug {
            return None;
        }
        self.theme = slug;
        self.wear()
    }

    pub(crate) fn set_background(&mut self, background: Option<Background>) -> Option<Theme> {
        if self.background == background {
            return None;
        }
        self.background = background;
        self.wear()
    }

    fn wear(&mut self) -> Option<Theme> {
        let theme = match (&self.theme, self.background) {
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
        };
        if theme == self.worn {
            return None;
        }
        self.worn = theme.clone();
        Some(theme)
    }
}
