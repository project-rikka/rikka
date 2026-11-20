use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Question {
    #[serde(default)]
    header: Option<String>,
    question: String,
    #[serde(default)]
    multi_select: Option<bool>,
    options: Vec<Choice>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Choice {
    label: String,
    #[serde(default)]
    description: Option<String>,
}

impl Question {
    #[must_use]
    pub fn new(question: impl Into<String>) -> Question {
        Question {
            header: None,
            question: question.into(),
            multi_select: None,
            options: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_header(mut self, header: impl Into<String>) -> Question {
        self.header = Some(header.into());
        self
    }

    #[must_use]
    pub fn option(mut self, label: impl Into<String>, description: impl Into<String>) -> Question {
        self.options.push(Choice {
            label: label.into(),
            description: Some(description.into()).filter(|text| !text.trim().is_empty()),
        });
        self
    }

    #[must_use]
    pub fn header(&self) -> &str {
        self.header.as_deref().unwrap_or_default()
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.question
    }

    #[must_use]
    pub fn is_multi_select(&self) -> bool {
        self.multi_select.unwrap_or(false)
    }

    #[must_use]
    pub fn options(&self) -> &[Choice] {
        &self.options
    }

    pub(crate) fn validation_error(&self) -> Option<String> {
        if self.question.trim().is_empty() {
            return Some("question text is empty".to_owned());
        }
        if self.options.len() < 2 {
            return Some(format!(
                "question \"{}\" has {} options, expected at least 2",
                self.question,
                self.options.len()
            ));
        }
        if let Some(blank) = self
            .options
            .iter()
            .position(|option| option.label.trim().is_empty())
        {
            return Some(format!(
                "option {} of \"{}\" has no label",
                blank + 1,
                self.question
            ));
        }

        let mut labels = std::collections::BTreeSet::new();
        self.options
            .iter()
            .find(|option| !labels.insert(option.label.trim()))
            .map(|duplicate| {
                format!(
                    "question \"{}\" repeats option \"{}\"",
                    self.question, duplicate.label
                )
            })
    }
}

impl Choice {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn description(&self) -> &str {
        self.description.as_deref().unwrap_or_default()
    }
}
