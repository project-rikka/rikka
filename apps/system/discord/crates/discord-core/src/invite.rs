use std::fmt;

const SCHEMES: [&str; 2] = ["https://", "http://"];
const SUBDOMAINS: [&str; 3] = ["www.", "ptb.", "canary."];
const HOSTS: [&str; 3] = [
    "discord.gg/",
    "discord.com/invite/",
    "discordapp.com/invite/",
];
const LONGEST: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InviteCode(String);

impl InviteCode {
    #[must_use]
    pub fn parse(input: &str) -> Option<InviteCode> {
        let mut rest = input.trim();
        rest = strip_any(rest, &SCHEMES).unwrap_or(rest);
        rest = strip_any(rest, &SUBDOMAINS).unwrap_or(rest);
        rest = strip_any(rest, &HOSTS).unwrap_or(rest);
        let code = rest.split(['?', '#']).next()?.trim_end_matches('/');
        let valid = !code.is_empty()
            && code.len() <= LONGEST
            && code
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
        valid.then(|| InviteCode(code.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InviteCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn strip_any<'a>(text: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().find_map(|prefix| {
        let head = text.get(..prefix.len())?;
        head.eq_ignore_ascii_case(prefix)
            .then(|| &text[prefix.len()..])
    })
}
