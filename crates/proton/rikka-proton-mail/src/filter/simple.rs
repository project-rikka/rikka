use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Operator {
    #[default]
    All,
    Any,
}

impl Operator {
    fn wire_names(self) -> (&'static str, &'static str) {
        match self {
            Operator::All => ("All", "all"),
            Operator::Any => ("Any", "any"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConditionKind {
    Subject,
    Sender,
    Recipient,
    Attachments,
}

impl ConditionKind {
    fn wire_names(self) -> (&'static str, &'static str) {
        match self {
            ConditionKind::Subject => ("Subject", "subject"),
            ConditionKind::Sender => ("Sender", "sender"),
            ConditionKind::Recipient => ("Recipient", "recipient"),
            ConditionKind::Attachments => ("Attachments", "attachments"),
        }
    }

    fn from_wire_value(value: &str) -> Option<ConditionKind> {
        [
            ConditionKind::Subject,
            ConditionKind::Sender,
            ConditionKind::Recipient,
            ConditionKind::Attachments,
        ]
        .into_iter()
        .find(|kind| kind.wire_names().1 == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Comparator {
    Contains,
    Is,
    Matches,
    Starts,
    Ends,
}

impl Comparator {
    fn wire_names(self, negated: bool) -> (&'static str, &'static str) {
        match (self, negated) {
            (Comparator::Contains, false) => ("contains", "contains"),
            (Comparator::Is, false) => ("is exactly", "is"),
            (Comparator::Matches, false) => ("matches", "matches"),
            (Comparator::Starts, false) => ("begins with", "starts"),
            (Comparator::Ends, false) => ("ends with", "ends"),
            (Comparator::Contains, true) => ("does not contain", "!contains"),
            (Comparator::Is, true) => ("is not", "!is"),
            (Comparator::Matches, true) => ("does not match", "!matches"),
            (Comparator::Starts, true) => ("does not begin with", "!starts"),
            (Comparator::Ends, true) => ("does not end with", "!ends"),
        }
    }

    fn from_wire_value(value: &str) -> Option<(Comparator, bool)> {
        let (negated, plain) = match value.strip_prefix('!') {
            Some(plain) => (true, plain),
            None => (false, value),
        };
        let comparator = match plain {
            "contains" => Comparator::Contains,
            "is" => Comparator::Is,
            "matches" => Comparator::Matches,
            "starts" => Comparator::Starts,
            "ends" => Comparator::Ends,
            _ => return None,
        };
        Some((comparator, negated))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    kind: ConditionKind,
    comparator: Comparator,
    negated: bool,
    values: Vec<String>,
}

impl Condition {
    #[must_use]
    pub fn new(
        kind: ConditionKind,
        comparator: Comparator,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Condition {
        Condition {
            kind,
            comparator,
            negated: false,
            values: values.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn subject(
        comparator: Comparator,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Condition {
        Condition::new(ConditionKind::Subject, comparator, values)
    }

    #[must_use]
    pub fn sender(
        comparator: Comparator,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Condition {
        Condition::new(ConditionKind::Sender, comparator, values)
    }

    #[must_use]
    pub fn recipient(
        comparator: Comparator,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Condition {
        Condition::new(ConditionKind::Recipient, comparator, values)
    }

    #[must_use]
    pub fn has_attachments() -> Condition {
        Condition::new(
            ConditionKind::Attachments,
            Comparator::Contains,
            Vec::<String>::new(),
        )
    }

    #[must_use]
    pub fn negated(mut self) -> Condition {
        self.negated = !self.negated;
        self
    }

    #[must_use]
    pub fn kind(&self) -> ConditionKind {
        self.kind
    }

    #[must_use]
    pub fn comparator(&self) -> Comparator {
        self.comparator
    }

    #[must_use]
    pub fn is_negated(&self) -> bool {
        self.negated
    }

    #[must_use]
    pub fn values(&self) -> &[String] {
        &self.values
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterActions {
    file_into: Vec<String>,
    read: bool,
    starred: bool,
    auto_reply: Option<String>,
}

impl FilterActions {
    #[must_use]
    pub fn folders(&self) -> &[String] {
        &self.file_into
    }

    #[must_use]
    pub fn marks_read(&self) -> bool {
        self.read
    }

    #[must_use]
    pub fn marks_starred(&self) -> bool {
        self.starred
    }

    #[must_use]
    pub fn auto_reply(&self) -> Option<&str> {
        self.auto_reply.as_deref()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.file_into.is_empty() && !self.read && !self.starred && self.auto_reply.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SimpleFilter {
    operator: Operator,
    conditions: Vec<Condition>,
    actions: FilterActions,
}

impl SimpleFilter {
    #[must_use]
    pub fn all() -> SimpleFilter {
        SimpleFilter::default()
    }

    #[must_use]
    pub fn any() -> SimpleFilter {
        SimpleFilter {
            operator: Operator::Any,
            ..SimpleFilter::default()
        }
    }

    #[must_use]
    pub fn when(mut self, condition: Condition) -> SimpleFilter {
        self.conditions.push(condition);
        self
    }

    #[must_use]
    pub fn file_into(mut self, folder: impl Into<String>) -> SimpleFilter {
        let folder = folder.into();
        if !self.actions.file_into.contains(&folder) {
            self.actions.file_into.push(folder);
        }
        self
    }

    #[must_use]
    pub fn mark_read(mut self) -> SimpleFilter {
        self.actions.read = true;
        self
    }

    #[must_use]
    pub fn mark_starred(mut self) -> SimpleFilter {
        self.actions.starred = true;
        self
    }

    #[must_use]
    pub fn auto_reply(mut self, html: impl Into<String>) -> SimpleFilter {
        self.actions.auto_reply = Some(html.into());
        self
    }

    #[must_use]
    pub fn operator(&self) -> Operator {
        self.operator
    }

    #[must_use]
    pub fn conditions(&self) -> &[Condition] {
        &self.conditions
    }

    #[must_use]
    pub fn actions(&self) -> &FilterActions {
        &self.actions
    }

    pub(crate) fn to_wire(&self) -> WireSimpleFilter {
        let (label, value) = self.operator.wire_names();
        WireSimpleFilter {
            operator: Choice::new(label, value),
            conditions: self
                .conditions
                .iter()
                .map(|condition| {
                    let (type_label, type_value) = condition.kind.wire_names();
                    let (comparator_label, comparator_value) =
                        condition.comparator.wire_names(condition.negated);
                    WireCondition {
                        kind: Choice::new(type_label, type_value),
                        comparator: Choice::new(comparator_label, comparator_value),
                        values: condition.values.clone(),
                    }
                })
                .collect(),
            actions: WireActions {
                file_into: self.actions.file_into.clone(),
                mark: WireMark {
                    read: self.actions.read,
                    starred: self.actions.starred,
                },
                vacation: self.actions.auto_reply.clone(),
            },
        }
    }

    pub(crate) fn from_wire(value: &Value) -> Option<SimpleFilter> {
        let wire_filter: WireSimpleFilter = serde_json::from_value(value.clone()).ok()?;

        let operator = match wire_filter.operator.value.as_str() {
            "any" => Operator::Any,
            "all" => Operator::All,
            _ => return None,
        };
        let conditions = wire_filter
            .conditions
            .into_iter()
            .map(|condition| {
                let kind = ConditionKind::from_wire_value(&condition.kind.value)?;
                let (comparator, negated) =
                    Comparator::from_wire_value(&condition.comparator.value)?;
                Some(Condition {
                    kind,
                    comparator,
                    negated,
                    values: condition.values,
                })
            })
            .collect::<Option<Vec<_>>>()?;

        Some(SimpleFilter {
            operator,
            conditions,
            actions: FilterActions {
                file_into: wire_filter.actions.file_into,
                read: wire_filter.actions.mark.read,
                starred: wire_filter.actions.mark.starred,
                auto_reply: wire_filter
                    .actions
                    .vacation
                    .filter(|reply| !reply.is_empty()),
            },
        })
    }
}

#[derive(Serialize, Deserialize)]
struct Choice {
    label: String,
    value: String,
}

impl Choice {
    fn new(label: &str, value: &str) -> Choice {
        Choice {
            label: label.to_owned(),
            value: value.to_owned(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct WireSimpleFilter {
    #[serde(rename = "Operator")]
    operator: Choice,
    #[serde(rename = "Conditions", default)]
    conditions: Vec<WireCondition>,
    #[serde(rename = "Actions")]
    actions: WireActions,
}

#[derive(Serialize, Deserialize)]
struct WireCondition {
    #[serde(rename = "Type")]
    kind: Choice,
    #[serde(rename = "Comparator")]
    comparator: Choice,
    #[serde(rename = "Values", default)]
    values: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct WireActions {
    #[serde(rename = "FileInto", default)]
    file_into: Vec<String>,
    #[serde(rename = "Mark", default)]
    mark: WireMark,
    #[serde(rename = "Vacation", default)]
    vacation: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct WireMark {
    #[serde(rename = "Read", default)]
    read: bool,
    #[serde(rename = "Starred", default)]
    starred: bool,
}
