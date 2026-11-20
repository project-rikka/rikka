use std::rc::Rc;

use rikka_image_edit::Histogram;
use rikka_types::Color;
use rikka_ui::{Element, Label, Space, Styled, TextStyled, View, text, tr};
use rikka_widgets::{NumberField, checkbox, color_well, combo, row, segmented};

use crate::editor::Dial;
use rikka_effects::{Knob, KnobKey, Scale};

use crate::form::Form;
use crate::rows::{self, FIELD};
use crate::scale::ScaleControls;
use crate::tuning::{Change, Tuning};

pub(crate) struct Tuner<S, P> {
    tuning: Tuning<S>,
    current: P,
}

impl<S, P: Clone> Clone for Tuner<S, P> {
    fn clone(&self) -> Tuner<S, P> {
        Tuner {
            tuning: self.tuning.clone(),
            current: self.current.clone(),
        }
    }
}

impl<S: 'static, P: Form> Tuner<S, P> {
    pub(crate) fn new(tuning: &Tuning<S>, current: &P) -> Tuner<S, P> {
        Tuner {
            tuning: tuning.clone(),
            current: current.clone(),
        }
    }

    pub(crate) fn form(&self) -> Element {
        P::form(self)
    }

    pub(crate) fn current(&self) -> &P {
        &self.current
    }

    pub(crate) fn histogram(&self) -> Option<&Histogram> {
        self.tuning.histogram.as_ref()
    }

    pub(crate) fn commit<A: 'static>(
        &self,
        change: impl Fn(&mut P, A) + 'static,
    ) -> impl Fn(&mut S, A) + Clone + 'static {
        self.sending(&self.tuning.commit, change)
    }

    pub(crate) fn preview<A: 'static>(
        &self,
        change: impl Fn(&mut P, A) + 'static,
    ) -> impl Fn(&mut S, A) + Clone + 'static {
        self.sending(&self.tuning.preview, change)
    }

    fn sending<A: 'static>(
        &self,
        to: &Change<S>,
        change: impl Fn(&mut P, A) + 'static,
    ) -> impl Fn(&mut S, A) + Clone + 'static {
        let current = self.current.clone();
        let to = Rc::clone(to);
        let change = Rc::new(change);
        move |host: &mut S, value: A| {
            let mut changed = current.clone();
            change(&mut changed, value);
            to(host, changed.into());
        }
    }

    pub(crate) fn staged<A: 'static>(
        &self,
        change: impl Fn(&mut P, A) + 'static,
    ) -> (
        impl Fn(&mut S, A) + Clone + 'static,
        impl Fn(&mut S, A) + Clone + 'static,
    ) {
        let change = Rc::new(change);
        let previewing = Rc::clone(&change);
        (
            self.preview(move |parameters: &mut P, value| previewing(parameters, value)),
            self.commit(move |parameters: &mut P, value| change(parameters, value)),
        )
    }

    pub(crate) fn tap(&self, change: impl Fn(&mut P) + 'static) -> impl Fn(&mut S) + 'static {
        let commit = self.commit(move |parameters: &mut P, (): ()| change(parameters));
        move |host: &mut S| commit(host, ())
    }

    pub(crate) fn value(&self, key: KnobKey<P>) -> f32 {
        self.current.peek(key.name()).unwrap_or_default()
    }

    fn knob(&self, key: KnobKey<P>) -> &'static Knob {
        self.current
            .knob(key.name())
            .expect("knob key not declared by its effect")
    }

    fn setting(key: KnobKey<P>) -> impl Fn(&mut P, f32) + Clone + 'static {
        move |parameters: &mut P, value: f32| {
            parameters.set(key.name(), value);
        }
    }

    pub(crate) fn adornment(&self, key: KnobKey<P>) -> Option<Element> {
        self.tuning
            .adorn
            .as_ref()
            .and_then(|adorn| adorn(key.name()))
    }

    pub(crate) fn submitting(&self, field: NumberField) -> NumberField {
        match &self.tuning.submit {
            Some(submit) => {
                let submit = Rc::clone(submit);
                field.on_submit(move |host: &mut S| submit(host))
            }
            None => field,
        }
    }

    pub(crate) fn number(&self, value: f32, scale: &Scale) -> NumberField {
        self.submitting(scale.field(value))
    }

    pub(crate) fn field(&self, key: KnobKey<P>) -> NumberField {
        let knob = self.knob(key);
        self.number(self.value(key), &knob.scale)
            .on_commit(self.commit(Self::setting(key)))
    }

    pub(crate) fn fielded(&self, label: Label, key: KnobKey<P>) -> Element {
        let field = self.field(key).width(FIELD);
        rows::labelled(
            label,
            row! {
                field;
                self.adornment(key);
            }
            .centered()
            .gap(Space::SNUG),
        )
    }

    pub(crate) fn sliding(&self, key: KnobKey<P>) -> Element {
        let knob = self.knob(key);
        let value = self.value(key);
        let setting = Self::setting(key);
        row! {
            knob.scale
                .slider(value)
                .on_change(self.preview(setting.clone()))
                .on_commit(self.commit(setting))
                .grow(1.0);
            self.field(key).width(FIELD);
            self.adornment(key);
        }
        .centered()
        .gap(Space::SNUG)
        .fill_width()
        .any()
    }

    pub(crate) fn amount(&self, key: KnobKey<P>) -> Element {
        self.amount_as((self.knob(key).label)(), key)
    }

    pub(crate) fn amount_as(&self, label: Label, key: KnobKey<P>) -> Element {
        rows::captioned(label, self.sliding(key))
    }

    pub(crate) fn between(&self, ends: (Label, Label), key: KnobKey<P>) -> Element {
        rows::ends(ends, self.sliding(key))
    }

    pub(crate) fn angle(&self, key: KnobKey<P>, folded: bool) -> Element {
        let knob = self.knob(key);
        let degrees = self.value(key);
        let setting = Self::setting(key);
        let dial = Dial::new(
            degrees,
            self.preview(setting.clone()),
            self.commit(setting.clone()),
        );
        let dial = match folded {
            true => dial.folded(),
            false => dial,
        };
        let field = self
            .number(degrees, &knob.scale)
            .on_commit(self.commit(setting))
            .width(FIELD);
        rows::captioned(
            (knob.label)(),
            row! {
                dial;
                field;
                self.adornment(key);
            }
            .centered()
            .gap(Space::NORMAL)
            .any(),
        )
    }

    pub(crate) fn toggle(
        &self,
        label: Label,
        on: bool,
        change: impl Fn(&mut P, bool) + 'static,
    ) -> Element {
        checkbox(on)
            .label(text(label))
            .on_toggle(self.commit(change))
            .any()
    }

    pub(crate) fn choice(
        &self,
        label: Label,
        at: Option<usize>,
        options: Vec<Label>,
        change: impl Fn(&mut P, usize) + 'static,
    ) -> Element {
        rows::labelled(
            label,
            combo(at)
                .options(options)
                .on_select(self.commit(change))
                .fill_width(),
        )
    }

    pub(crate) fn preset(
        &self,
        label: Label,
        at: Option<usize>,
        options: Vec<Label>,
        change: impl Fn(&mut P, usize) + 'static,
    ) -> Element {
        rows::labelled(
            label,
            combo(at)
                .options(options)
                .placeholder(tr!("effects.curves.custom"))
                .on_select(self.commit(change))
                .fill_width(),
        )
    }

    pub(crate) fn segments(
        &self,
        at: usize,
        labels: Vec<Label>,
        change: impl Fn(&mut P, usize) + 'static,
    ) -> Element {
        segmented(at)
            .segments(labels)
            .on_select(self.commit(change))
            .fill_width()
            .any()
    }

    pub(crate) fn swatch(&self, color: Color, change: impl Fn(&mut P, Color) + 'static) -> Element {
        let (preview, commit) = self.staged(change);
        row! {
            color_well(color).on_change(preview).on_commit(commit);
            text(rows::color_name(color)).caption().muted().no_wrap();
        }
        .centered()
        .gap(Space::SNUG)
        .any()
    }
}
