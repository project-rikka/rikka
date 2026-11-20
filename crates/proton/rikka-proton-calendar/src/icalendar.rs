use std::fmt;

use rikka_proton_contentline::ContentLine;

use crate::component::Component;
use crate::error::{Error, Result};
use crate::event::{Event, VEVENT};
use crate::zones::Zones;

const VCALENDAR: &str = "VCALENDAR";
const VTIMEZONE: &str = "VTIMEZONE";
const VERSION: &str = "VERSION";
const PRODID: &str = "PRODID";
const METHOD: &str = "METHOD";
const CALSCALE: &str = "CALSCALE";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Method {
    Publish,
    Request,
    Reply,
    Add,
    Cancel,
    Refresh,
    Counter,
    DeclineCounter,
    Other(String),
}

impl Method {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Method::Publish => "PUBLISH",
            Method::Request => "REQUEST",
            Method::Reply => "REPLY",
            Method::Add => "ADD",
            Method::Cancel => "CANCEL",
            Method::Refresh => "REFRESH",
            Method::Counter => "COUNTER",
            Method::DeclineCounter => "DECLINECOUNTER",
            Method::Other(other) => other,
        }
    }
}

impl From<&str> for Method {
    fn from(text: &str) -> Method {
        match text.trim().to_ascii_uppercase().as_str() {
            "PUBLISH" => Method::Publish,
            "REQUEST" => Method::Request,
            "REPLY" => Method::Reply,
            "ADD" => Method::Add,
            "CANCEL" => Method::Cancel,
            "REFRESH" => Method::Refresh,
            "COUNTER" => Method::Counter,
            "DECLINECOUNTER" => Method::DeclineCounter,
            other => Method::Other(other.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ICalendar {
    component: Component,
}

impl ICalendar {
    #[must_use]
    pub fn new(prod_id: &str) -> ICalendar {
        let mut component = Component::new(VCALENDAR);
        component.push(ContentLine::new(VERSION, "2.0"));
        component.push(ContentLine::new(PRODID, prod_id));
        component.push(ContentLine::new(CALSCALE, "GREGORIAN"));
        ICalendar { component }
    }

    pub fn parse(text: &str) -> Result<ICalendar> {
        ICalendar::parse_all(text)?
            .into_iter()
            .next()
            .ok_or(Error::NoCalendar)
    }

    pub fn parse_all(text: &str) -> Result<Vec<ICalendar>> {
        Ok(Component::parse_all(text)?
            .into_iter()
            .filter(|component| component.is(VCALENDAR))
            .map(|component| ICalendar { component })
            .collect())
    }

    pub fn from_component(component: Component) -> Result<ICalendar> {
        if !component.is(VCALENDAR) {
            return Err(Error::WrongComponent(
                VCALENDAR,
                component.name().to_owned(),
            ));
        }
        Ok(ICalendar { component })
    }

    #[must_use]
    pub fn with_method(mut self, method: &Method) -> ICalendar {
        self.set_method(Some(method));
        self
    }

    #[must_use]
    pub fn with_event(mut self, event: Event) -> ICalendar {
        self.push_event(event);
        self
    }

    pub fn set_method(&mut self, method: Option<&Method>) {
        match method {
            Some(method) => self
                .component
                .set(ContentLine::new(METHOD, method.as_str())),
            None => self.component.remove(METHOD),
        }
    }

    #[must_use]
    pub fn method(&self) -> Option<Method> {
        self.component
            .property(METHOD)
            .map(|line| Method::from(line.value()))
    }

    #[must_use]
    pub fn prod_id(&self) -> Option<String> {
        self.component.text(PRODID)
    }

    pub fn events(&self) -> Result<Vec<Event>> {
        self.component
            .children(VEVENT)
            .cloned()
            .map(Event::from_component)
            .collect()
    }

    pub fn timezones(&self) -> impl Iterator<Item = &Component> {
        self.component.children(VTIMEZONE)
    }

    pub fn zones(&self) -> Result<Zones> {
        let mut zones = Zones::new();
        zones.define_all(self.timezones())?;
        Ok(zones)
    }

    pub fn push_event(&mut self, event: Event) {
        self.component.push_component(event.into_component());
    }

    pub fn push_timezone(&mut self, timezone: Component) {
        let insert_at = self
            .component
            .components()
            .iter()
            .position(|component| !component.is(VTIMEZONE))
            .unwrap_or(self.component.components().len());
        self.component.components_mut().insert(insert_at, timezone);
    }

    #[must_use]
    pub fn component(&self) -> &Component {
        &self.component
    }

    pub fn component_mut(&mut self) -> &mut Component {
        &mut self.component
    }

    #[must_use]
    pub fn into_component(self) -> Component {
        self.component
    }
}

impl fmt::Display for ICalendar {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.component.fmt(formatter)
    }
}
