use std::fmt::{self, Write as _};

use crate::filter::simple::{Comparator, Condition, ConditionKind, Operator, SimpleFilter};

const ENVIRONMENT: &str = r#"require ["include", "environment", "variables", "relational", "comparator-i;ascii-numeric", "spamtest"];"#;
const SPAM_GUARD: &str = r#"# Generated: Do not run this script on spam messages
if allof (environment :matches "vnd.proton.spam-threshold" "*",
spamtest :value "ge" :comparator "i;ascii-numeric" "${1}")
{
    return;
}"#;
const CASEMAP: &str = r#":comparator "i;unicode-casemap""#;
const ATTACHED: &str = "X-Attached";
const RECIPIENTS: &str = r#"["To", "Cc", "Bcc"]"#;
const AUTO_REPLY_HEAD: &str = "MIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n";

impl SimpleFilter {
    #[must_use]
    pub fn to_sieve(&self) -> String {
        SieveWriter(self).to_string()
    }
}

struct SieveWriter<'a>(&'a SimpleFilter);

impl fmt::Display for SieveWriter<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let filter = self.0;
        let actions = filter.actions();
        let mut extensions = Vec::new();
        if !actions.folders().is_empty() {
            extensions.push("\"fileinto\"");
        }
        if actions.marks_read() || actions.marks_starred() {
            extensions.push("\"imap4flags\"");
        }
        if actions.auto_reply().is_some() {
            extensions.push("\"vacation\"");
        }

        writeln!(out, "{ENVIRONMENT}")?;
        if !extensions.is_empty() {
            writeln!(out, "require [{}];", extensions.join(", "))?;
        }
        write!(out, "\n{SPAM_GUARD}\n\n\n")?;

        let tests: Vec<String> = filter.conditions().iter().filter_map(sieve_test).collect();
        let test = match (tests.len(), filter.operator()) {
            (0, _) => "true".to_owned(),
            (_, Operator::All) => format!("allof ({})", tests.join(", ")),
            (_, Operator::Any) => format!("anyof ({})", tests.join(", ")),
        };
        write!(out, "if {test}\n{{\n")?;

        for folder in actions.folders() {
            writeln!(out, "    fileinto {};", Quoted(folder))?;
        }
        if actions.marks_read() {
            writeln!(out, "    addflag \"\\\\Seen\";")?;
        }
        if actions.marks_starred() {
            writeln!(out, "    addflag \"\\\\Flagged\";")?;
        }
        if let Some(reply) = actions.auto_reply() {
            let body = format!("{AUTO_REPLY_HEAD}{reply}");
            writeln!(out, "    vacation :mime {};", Quoted(&body))?;
        }

        writeln!(out, "}}")
    }
}

fn sieve_test(condition: &Condition) -> Option<String> {
    let test = match condition.kind() {
        ConditionKind::Attachments => format!("exists {}", Quoted(ATTACHED)),
        kind => {
            if condition.values().is_empty() {
                return None;
            }
            let (tag, patterns) = patterns(condition.comparator(), condition.values());
            let values = listed(&patterns);
            match kind {
                ConditionKind::Subject => {
                    format!("header {CASEMAP} :{tag} \"Subject\" {values}")
                }
                ConditionKind::Sender => {
                    format!("address :all {CASEMAP} :{tag} \"From\" {values}")
                }
                _ => format!("address :all {CASEMAP} :{tag} {RECIPIENTS} {values}"),
            }
        }
    };

    Some(match condition.is_negated() {
        true => format!("not {test}"),
        false => test,
    })
}

fn patterns(comparator: Comparator, values: &[String]) -> (&'static str, Vec<String>) {
    match comparator {
        Comparator::Contains => ("contains", values.to_vec()),
        Comparator::Is => ("is", values.to_vec()),
        Comparator::Matches => ("matches", values.to_vec()),
        Comparator::Starts => (
            "matches",
            values
                .iter()
                .map(|value| format!("{}*", wildcard_escaped(value)))
                .collect(),
        ),
        Comparator::Ends => (
            "matches",
            values
                .iter()
                .map(|value| format!("*{}", wildcard_escaped(value)))
                .collect(),
        ),
    }
}

pub(crate) fn wildcard_escaped(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '*' | '?' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn listed(values: &[String]) -> String {
    match values {
        [single] => Quoted(single).to_string(),
        many => format!(
            "[{}]",
            many.iter()
                .map(|value| Quoted(value).to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

struct Quoted<'a>(&'a str);

impl fmt::Display for Quoted<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_char('"')?;
        for character in self.0.chars() {
            if character == '"' || character == '\\' {
                out.write_char('\\')?;
            }
            out.write_char(character)?;
        }
        out.write_char('"')
    }
}
