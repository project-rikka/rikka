mod known;
mod line;

pub(crate) use line::look;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inspection {
    Read,
    Search,
    List,
}

impl Inspection {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Inspection::Read => "Read",
            Inspection::Search => "Search",
            Inspection::List => "List",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Inspection::List => 0,
            Inspection::Read => 1,
            Inspection::Search => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Look {
    inspection: Inspection,
    subject: String,
}

impl Look {
    #[must_use]
    pub fn inspection(&self) -> Inspection {
        self.inspection
    }

    #[must_use]
    pub fn subject(&self) -> Option<&str> {
        Some(self.subject.as_str()).filter(|subject| !subject.is_empty())
    }

    fn and(self, other: Look) -> Look {
        if self.inspection == other.inspection {
            return Look {
                inspection: self.inspection,
                subject: beside(self.subject, other.subject),
            };
        }
        match other.inspection.rank() > self.inspection.rank() {
            true => other,
            false => self,
        }
    }
}

fn beside(first: String, second: String) -> String {
    match (first.is_empty(), second.is_empty()) {
        (true, _) => second,
        (_, true) => first,
        _ => format!("{first}, {second}"),
    }
}
