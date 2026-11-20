use rikka_proton_contentline::{
    ContentLine, Parameter, escape_text, split_unescaped, unescape_text,
};

const PREF: &str = "PREF";
const TYPE: &str = "TYPE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    line: ContentLine,
}

impl Property {
    #[must_use]
    pub fn text(name: impl AsRef<str>, text: &str) -> Property {
        Property::raw(name, escape_text(text))
    }

    #[must_use]
    pub fn raw(name: impl AsRef<str>, raw: impl Into<String>) -> Property {
        Property {
            line: ContentLine::new(name, raw),
        }
    }

    #[must_use]
    pub fn list<S: AsRef<str>>(name: impl AsRef<str>, items: &[S]) -> Property {
        let raw = items
            .iter()
            .map(|item| escape_text(item.as_ref()))
            .collect::<Vec<_>>()
            .join(",");
        Property::raw(name, raw)
    }

    #[must_use]
    pub fn structured<S: AsRef<str>>(name: impl AsRef<str>, components: &[Vec<S>]) -> Property {
        let raw = components
            .iter()
            .map(|component| {
                component
                    .iter()
                    .map(|item| escape_text(item.as_ref()))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect::<Vec<_>>()
            .join(";");
        Property::raw(name, raw)
    }

    #[must_use]
    pub fn from_line(line: ContentLine) -> Property {
        Property { line }
    }

    #[must_use]
    pub fn with_group(mut self, group: impl Into<String>) -> Property {
        self.line = self.line.with_group(group);
        self
    }

    #[must_use]
    pub fn with_parameter(mut self, name: impl AsRef<str>, value: impl Into<String>) -> Property {
        self.line.set_parameter(Parameter::new(name, value));
        self
    }

    #[must_use]
    pub fn with_types<S: Into<String>>(mut self, types: impl IntoIterator<Item = S>) -> Property {
        self.line.set_parameter(Parameter::with_values(TYPE, types));
        self
    }

    #[must_use]
    pub fn with_pref(self, pref: u32) -> Property {
        self.with_parameter(PREF, pref.to_string())
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.line.name()
    }

    #[must_use]
    pub fn is(&self, name: &str) -> bool {
        self.line.is(name)
    }

    #[must_use]
    pub fn group(&self) -> Option<&str> {
        self.line.group()
    }

    #[must_use]
    pub fn in_group(&self, group: &str) -> bool {
        self.group()
            .is_some_and(|member| member.eq_ignore_ascii_case(group))
    }

    #[must_use]
    pub fn raw_value(&self) -> &str {
        self.line.value()
    }

    #[must_use]
    pub fn text_value(&self) -> String {
        unescape_text(self.line.value())
    }

    #[must_use]
    pub fn list_value(&self) -> Vec<String> {
        split_unescaped(self.line.value(), ',')
            .into_iter()
            .map(unescape_text)
            .collect()
    }

    #[must_use]
    pub fn components(&self) -> Vec<Vec<String>> {
        split_unescaped(self.line.value(), ';')
            .into_iter()
            .map(|component| {
                split_unescaped(component, ',')
                    .into_iter()
                    .map(unescape_text)
                    .collect()
            })
            .collect()
    }

    #[must_use]
    pub fn parameters(&self) -> &[Parameter] {
        self.line.parameters()
    }

    #[must_use]
    pub fn parameter(&self, name: &str) -> Option<&Parameter> {
        self.line.parameter(name)
    }

    #[must_use]
    pub fn parameter_value(&self, name: &str) -> Option<&str> {
        self.line.parameter_value(name)
    }

    #[must_use]
    pub fn types(&self) -> Vec<String> {
        self.parameter(TYPE)
            .map(|parameter| {
                parameter
                    .values()
                    .iter()
                    .flat_map(|value| value.split(','))
                    .map(|value| value.trim().to_ascii_lowercase())
                    .filter(|value| !value.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[must_use]
    pub fn pref(&self) -> Option<u32> {
        self.parameter_value(PREF)
            .and_then(|pref| pref.trim().parse().ok())
    }

    #[must_use]
    pub fn line(&self) -> &ContentLine {
        &self.line
    }

    pub fn set_group(&mut self, group: Option<String>) {
        self.line.set_group(group);
    }

    pub fn set_raw_value(&mut self, raw: impl Into<String>) {
        self.line.set_value(raw);
    }

    pub fn set_text_value(&mut self, text: &str) {
        self.line.set_value(escape_text(text));
    }

    pub fn set_parameter(&mut self, parameter: Parameter) {
        self.line.set_parameter(parameter);
    }

    pub fn set_pref(&mut self, pref: u32) {
        self.line
            .set_parameter(Parameter::new(PREF, pref.to_string()));
    }

    pub fn remove_parameter(&mut self, name: &str) -> Option<Parameter> {
        self.line.remove_parameter(name)
    }
}
