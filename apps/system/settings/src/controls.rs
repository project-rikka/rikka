use std::ops::RangeInclusive;

use rikka_app::prelude::*;

use crate::amount::amount;
use crate::domain::DomainPage;
use crate::picker::choose;

pub(crate) type Choices = &'static [(&'static str, &'static str)];

pub(crate) fn flag<P: DomainPage>(page: &P, label: Label, key: &'static str) -> Field {
    flag_or(page, label, key, false)
}

pub(crate) fn flag_or<P: DomainPage>(
    page: &P,
    label: Label,
    key: &'static str,
    absent: bool,
) -> Field {
    field(
        label,
        switch(page.domain().get(key).unwrap_or(absent))
            .on_toggle(move |page: &mut P, on: bool| page.domain_mut().set(key, on)),
    )
}

pub(crate) fn choice<P: DomainPage>(
    page: &P,
    label: Label,
    key: &'static str,
    options: Choices,
) -> Field {
    field(
        label,
        choose(
            options,
            page.domain().one_of(key, options),
            move |page: &mut P, value: &'static str| page.domain_mut().set(key, value),
        ),
    )
}

pub(crate) fn span(range: &RangeInclusive<i64>) -> RangeInclusive<f32> {
    *range.start() as f32..=*range.end() as f32
}

pub(crate) struct Count {
    pub(crate) key: &'static str,
    pub(crate) absent: i64,
    pub(crate) range: RangeInclusive<f32>,
}

impl Count {
    pub(crate) fn field<P: DomainPage>(
        self,
        page: &P,
        label: Label,
        reading: impl FnOnce(i64) -> String,
    ) -> Field {
        self.field_in_steps(page, label, 1, reading)
    }

    pub(crate) fn field_in_steps<P: DomainPage>(
        self,
        page: &P,
        label: Label,
        step: i64,
        reading: impl FnOnce(i64) -> String,
    ) -> Field {
        let Count { key, absent, range } = self;
        let value = page.domain().get(key).unwrap_or(absent);
        field(
            label,
            amount(
                value as f32,
                range,
                step as f32,
                reading(value),
                move |page: &mut P, moved: f32| page.domain_mut().set(key, moved.round() as i64),
            ),
        )
    }
}
