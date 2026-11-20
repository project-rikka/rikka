use rikka_proton_contentline::{ContentLine, Parameter};

use crate::component::Component;
use crate::date_value::DateValue;
use crate::duration::Duration;
use crate::error::{Error, Result};

pub(crate) const VALARM: &str = "VALARM";
const ACTION: &str = "ACTION";
const TRIGGER: &str = "TRIGGER";
const RELATED: &str = "RELATED";
const VALUE: &str = "VALUE";
const DESCRIPTION: &str = "DESCRIPTION";
const SUMMARY: &str = "SUMMARY";
const ATTENDEE: &str = "ATTENDEE";
const REPEAT: &str = "REPEAT";
const DURATION: &str = "DURATION";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AlarmAction {
    Display,
    Email,
    Audio,
    Other(String),
}

impl AlarmAction {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            AlarmAction::Display => "DISPLAY",
            AlarmAction::Email => "EMAIL",
            AlarmAction::Audio => "AUDIO",
            AlarmAction::Other(other) => other,
        }
    }
}

impl From<&str> for AlarmAction {
    fn from(text: &str) -> AlarmAction {
        match text.trim().to_ascii_uppercase().as_str() {
            "DISPLAY" => AlarmAction::Display,
            "EMAIL" => AlarmAction::Email,
            "AUDIO" => AlarmAction::Audio,
            other => AlarmAction::Other(other.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Trigger {
    Relative { duration: Duration, from_end: bool },
    Absolute(DateValue),
}

impl Trigger {
    #[must_use]
    pub fn before_start(duration: Duration) -> Trigger {
        Trigger::Relative {
            duration: Duration {
                negative: true,
                ..duration
            },
            from_end: false,
        }
    }

    pub fn from_line(line: &ContentLine) -> Result<Trigger> {
        let absolute = line
            .parameter_value(VALUE)
            .is_some_and(|value| value.eq_ignore_ascii_case("DATE-TIME"));
        if absolute {
            return Ok(Trigger::Absolute(DateValue::from_line(line)?));
        }
        Ok(Trigger::Relative {
            duration: line.value().parse()?,
            from_end: line
                .parameter_value(RELATED)
                .is_some_and(|related| related.eq_ignore_ascii_case("END")),
        })
    }

    #[must_use]
    pub fn to_line(&self) -> ContentLine {
        match self {
            Trigger::Absolute(at) => at
                .to_line(TRIGGER)
                .with_parameter(Parameter::new(VALUE, "DATE-TIME")),
            Trigger::Relative { duration, from_end } => {
                let line = ContentLine::new(TRIGGER, duration.to_string());
                match from_end {
                    true => line.with_parameter(Parameter::new(RELATED, "END")),
                    false => line,
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alarm {
    component: Component,
}

impl Alarm {
    #[must_use]
    pub fn new(action: AlarmAction, trigger: Trigger) -> Alarm {
        let mut component = Component::new(VALARM);
        component.push(ContentLine::new(ACTION, action.as_str()));
        component.push(trigger.to_line());
        Alarm { component }
    }

    pub fn from_component(component: Component) -> Result<Alarm> {
        if !component.is(VALARM) {
            return Err(Error::WrongComponent(VALARM, component.name().to_owned()));
        }
        Ok(Alarm { component })
    }

    #[must_use]
    pub fn with_description(mut self, description: &str) -> Alarm {
        self.component.set_text(DESCRIPTION, description);
        self
    }

    #[must_use]
    pub fn with_summary(mut self, summary: &str) -> Alarm {
        self.component.set_text(SUMMARY, summary);
        self
    }

    #[must_use]
    pub fn with_attendee(mut self, email: &str) -> Alarm {
        self.component.push(ContentLine::new(
            ATTENDEE,
            format!("mailto:{}", email.trim()),
        ));
        self
    }

    #[must_use]
    pub fn action(&self) -> AlarmAction {
        self.component
            .property(ACTION)
            .map(|line| AlarmAction::from(line.value()))
            .unwrap_or(AlarmAction::Display)
    }

    pub fn trigger(&self) -> Result<Trigger> {
        Trigger::from_line(
            self.component
                .property(TRIGGER)
                .ok_or(Error::MissingProperty(VALARM, TRIGGER))?,
        )
    }

    #[must_use]
    pub fn description(&self) -> Option<String> {
        self.component.text(DESCRIPTION)
    }

    #[must_use]
    pub fn repeat(&self) -> Option<(u32, Duration)> {
        let count = self
            .component
            .property(REPEAT)?
            .value()
            .trim()
            .parse()
            .ok()?;
        let every = self.component.property(DURATION)?.value().parse().ok()?;
        Some((count, every))
    }

    #[must_use]
    pub fn component(&self) -> &Component {
        &self.component
    }

    #[must_use]
    pub fn into_component(self) -> Component {
        self.component
    }

    #[must_use]
    pub fn with_publish_fields(mut self, title: &str, email: &str) -> Alarm {
        match self.action() {
            AlarmAction::Display if self.component.property(DESCRIPTION).is_none() => {
                self.component.set_text(DESCRIPTION, title);
            }
            AlarmAction::Email => {
                if self.component.property(SUMMARY).is_none() {
                    self.component.set_text(SUMMARY, title);
                }
                if self.component.property(DESCRIPTION).is_none() {
                    self.component.set_text(DESCRIPTION, title);
                }
                if self.component.property(ATTENDEE).is_none() {
                    self.component.push(ContentLine::new(
                        ATTENDEE,
                        format!("mailto:{}", email.trim()),
                    ));
                }
            }
            _ => {}
        }
        self
    }
}
