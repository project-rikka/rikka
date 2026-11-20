use rikka_proton_contentline::{ContentLine, Parameter};

const ATTENDEE: &str = "ATTENDEE";
const ORGANIZER: &str = "ORGANIZER";
const CN: &str = "CN";
const ROLE: &str = "ROLE";
const PARTSTAT: &str = "PARTSTAT";
const RSVP: &str = "RSVP";
const TOKEN: &str = "X-PM-TOKEN";
const CUTYPE: &str = "CUTYPE";
const MAILTO: &str = "mailto:";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ParticipationStatus {
    #[default]
    NeedsAction,
    Accepted,
    Declined,
    Tentative,
    Delegated,
}

impl ParticipationStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            ParticipationStatus::NeedsAction => "NEEDS-ACTION",
            ParticipationStatus::Accepted => "ACCEPTED",
            ParticipationStatus::Declined => "DECLINED",
            ParticipationStatus::Tentative => "TENTATIVE",
            ParticipationStatus::Delegated => "DELEGATED",
        }
    }

    #[must_use]
    pub fn api_status(self) -> i32 {
        match self {
            ParticipationStatus::Tentative => 1,
            ParticipationStatus::Declined => 2,
            ParticipationStatus::Accepted => 3,
            ParticipationStatus::NeedsAction | ParticipationStatus::Delegated => 0,
        }
    }

    #[must_use]
    pub fn from_api_status(status: i32) -> ParticipationStatus {
        match status {
            1 => ParticipationStatus::Tentative,
            2 => ParticipationStatus::Declined,
            3 => ParticipationStatus::Accepted,
            _ => ParticipationStatus::NeedsAction,
        }
    }
}

impl From<&str> for ParticipationStatus {
    fn from(text: &str) -> ParticipationStatus {
        match text.trim().to_ascii_uppercase().as_str() {
            "ACCEPTED" => ParticipationStatus::Accepted,
            "DECLINED" => ParticipationStatus::Declined,
            "TENTATIVE" => ParticipationStatus::Tentative,
            "DELEGATED" => ParticipationStatus::Delegated,
            _ => ParticipationStatus::NeedsAction,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Role {
    Chair,
    #[default]
    Required,
    Optional,
    NonParticipant,
}

impl Role {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Chair => "CHAIR",
            Role::Required => "REQ-PARTICIPANT",
            Role::Optional => "OPT-PARTICIPANT",
            Role::NonParticipant => "NON-PARTICIPANT",
        }
    }
}

impl From<&str> for Role {
    fn from(text: &str) -> Role {
        match text.trim().to_ascii_uppercase().as_str() {
            "CHAIR" => Role::Chair,
            "OPT-PARTICIPANT" => Role::Optional,
            "NON-PARTICIPANT" => Role::NonParticipant,
            _ => Role::Required,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attendee {
    line: ContentLine,
}

impl Attendee {
    #[must_use]
    pub fn new(email: &str) -> Attendee {
        Attendee {
            line: ContentLine::new(ATTENDEE, mailto_uri(email)),
        }
        .with_name(email)
        .with_role(Role::Required)
        .with_status(ParticipationStatus::NeedsAction)
        .with_rsvp(true)
    }

    #[must_use]
    pub fn from_line(line: ContentLine) -> Attendee {
        Attendee { line }
    }

    #[must_use]
    pub fn with_name(mut self, name: &str) -> Attendee {
        self.line.set_parameter(Parameter::new(CN, name));
        self
    }

    #[must_use]
    pub fn with_role(mut self, role: Role) -> Attendee {
        self.line.set_parameter(Parameter::new(ROLE, role.as_str()));
        self
    }

    #[must_use]
    pub fn with_status(mut self, status: ParticipationStatus) -> Attendee {
        self.set_status(status);
        self
    }

    #[must_use]
    pub fn with_rsvp(mut self, rsvp: bool) -> Attendee {
        self.line
            .set_parameter(Parameter::new(RSVP, if rsvp { "TRUE" } else { "FALSE" }));
        self
    }

    #[must_use]
    pub fn with_token(mut self, token: impl Into<String>) -> Attendee {
        self.line.set_parameter(Parameter::new(TOKEN, token));
        self
    }

    pub fn set_status(&mut self, status: ParticipationStatus) {
        self.line
            .set_parameter(Parameter::new(PARTSTAT, status.as_str()));
    }

    #[must_use]
    pub fn email(&self) -> String {
        line_email(&self.line)
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.line.parameter_value(CN)
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.line
            .parameter_value(ROLE)
            .map(Role::from)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn status(&self) -> ParticipationStatus {
        self.line
            .parameter_value(PARTSTAT)
            .map(ParticipationStatus::from)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn rsvp(&self) -> bool {
        self.line
            .parameter_value(RSVP)
            .is_some_and(|rsvp| rsvp.eq_ignore_ascii_case("TRUE"))
    }

    #[must_use]
    pub fn token(&self) -> Option<&str> {
        self.line.parameter_value(TOKEN)
    }

    #[must_use]
    pub fn user_type(&self) -> Option<&str> {
        self.line.parameter_value(CUTYPE)
    }

    #[must_use]
    pub fn line(&self) -> &ContentLine {
        &self.line
    }

    #[must_use]
    pub fn into_line(self) -> ContentLine {
        self.line
    }

    pub(crate) fn without_parameter(&self, parameter: &str) -> Attendee {
        let mut line = self.line.clone();
        line.remove_parameter(parameter);
        Attendee { line }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Organizer {
    line: ContentLine,
}

impl Organizer {
    #[must_use]
    pub fn new(email: &str) -> Organizer {
        Organizer {
            line: ContentLine::new(ORGANIZER, mailto_uri(email)),
        }
    }

    #[must_use]
    pub fn from_line(line: ContentLine) -> Organizer {
        Organizer { line }
    }

    #[must_use]
    pub fn with_name(mut self, name: &str) -> Organizer {
        self.line.set_parameter(Parameter::new(CN, name));
        self
    }

    #[must_use]
    pub fn email(&self) -> String {
        line_email(&self.line)
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.line.parameter_value(CN)
    }

    #[must_use]
    pub fn line(&self) -> &ContentLine {
        &self.line
    }

    #[must_use]
    pub fn into_line(self) -> ContentLine {
        self.line
    }
}

fn mailto_uri(email: &str) -> String {
    format!("{MAILTO}{}", email.trim())
}

fn line_email(line: &ContentLine) -> String {
    let value = line.value().trim();
    let address = match value.get(..MAILTO.len()) {
        Some(prefix) if prefix.eq_ignore_ascii_case(MAILTO) => &value[MAILTO.len()..],
        _ => value,
    };
    let address = address.trim();
    if address.is_empty() {
        return line
            .parameter_value("EMAIL")
            .or_else(|| line.parameter_value(CN))
            .unwrap_or_default()
            .trim()
            .to_owned();
    }
    address.to_owned()
}
