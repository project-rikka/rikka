use std::rc::Rc;

use rikka_image_edit::Histogram;
use rikka_ui::Element;

use rikka_effects::Effect;

pub(crate) type Change<S> = Rc<dyn Fn(&mut S, Effect)>;
type Submit<S> = Rc<dyn Fn(&mut S)>;
type Adorn = Rc<dyn Fn(&'static str) -> Option<Element>>;

pub struct Tuning<S> {
    pub(crate) preview: Change<S>,
    pub(crate) commit: Change<S>,
    pub(crate) submit: Option<Submit<S>>,
    pub(crate) adorn: Option<Adorn>,
    pub(crate) histogram: Option<Histogram>,
}

impl<S: 'static> Tuning<S> {
    pub fn new(
        preview: impl Fn(&mut S, Effect) + 'static,
        commit: impl Fn(&mut S, Effect) + 'static,
    ) -> Tuning<S> {
        Tuning {
            preview: Rc::new(preview),
            commit: Rc::new(commit),
            submit: None,
            adorn: None,
            histogram: None,
        }
    }

    #[must_use]
    pub fn submit(mut self, submit: impl Fn(&mut S) + 'static) -> Tuning<S> {
        self.submit = Some(Rc::new(submit));
        self
    }

    #[must_use]
    pub fn adorn(mut self, adorn: impl Fn(&'static str) -> Option<Element> + 'static) -> Tuning<S> {
        self.adorn = Some(Rc::new(adorn));
        self
    }

    #[must_use]
    pub fn histogram(mut self, histogram: Option<Histogram>) -> Tuning<S> {
        self.histogram = histogram;
        self
    }
}

impl<S> Clone for Tuning<S> {
    fn clone(&self) -> Tuning<S> {
        Tuning {
            preview: Rc::clone(&self.preview),
            commit: Rc::clone(&self.commit),
            submit: self.submit.clone(),
            adorn: self.adorn.clone(),
            histogram: self.histogram.clone(),
        }
    }
}

impl<S> std::fmt::Debug for Tuning<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tuning")
            .field("submits", &self.submit.is_some())
            .field("adorns", &self.adorn.is_some())
            .field("histogram", &self.histogram.is_some())
            .finish_non_exhaustive()
    }
}
