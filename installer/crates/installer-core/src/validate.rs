use crate::{answers::Answers, catalog::PACKAGES_FLAKE, plan::PartitionPlan, step::Step};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub step: Step,
    pub message: String,
}

const RESERVED_USERNAMES: &[&str] = &[
    "root",
    "nobody",
    "daemon",
    "bin",
    "sys",
    "sync",
    "nixbld",
    "nscd",
    "systemd-network",
    "systemd-resolve",
    "messagebus",
    "sshd",
    "nixos",
];

#[must_use]
pub fn validate(answers: &Answers) -> Vec<Problem> {
    Step::ALL
        .iter()
        .take_while(|step| **step != Step::Summary)
        .flat_map(|step| validate_step(answers, *step))
        .collect()
}

#[must_use]
pub fn validate_step(answers: &Answers, step: Step) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut fail = |message: String| problems.push(Problem { step, message });

    match step {
        Step::Language | Step::Keyboard => {}

        Step::Timezone => {
            if answers.timezone.trim().is_empty() {
                fail("Choose a time zone.".to_owned());
            }
        }

        Step::Disk => {
            if let Err(error) = PartitionPlan::build(answers) {
                fail(error.to_string());
            }
            if let Some(disk) = &answers.disk {
                if disk.read_only {
                    fail(format!("{} is read-only.", disk.name));
                }
                if disk.in_use {
                    fail(format!(
                        "{} is mounted; it may be the medium you booted from.",
                        disk.name
                    ));
                }
            }
            if answers.encrypt
                && let Err(message) = check_secret(
                    answers.passphrase.as_str(),
                    answers.passphrase_confirm.as_str(),
                    "Passphrase",
                )
            {
                fail(message);
            }
        }

        Step::Account => {
            if let Err(message) = check_hostname(&answers.hostname) {
                fail(message);
            }
            if let Err(message) = check_username(&answers.username) {
                fail(message);
            }
            if let Err(message) = check_secret(
                answers.password.as_str(),
                answers.password_confirm.as_str(),
                "Password",
            ) {
                fail(message);
            }
            if answers.set_root_password
                && let Err(message) = check_secret(
                    answers.root_password.as_str(),
                    answers.root_password_confirm.as_str(),
                    "Root password",
                )
            {
                fail(message);
            }
        }

        Step::System => {
            if answers.nvidia_driver().is_some() && !answers.allow_unfree {
                fail("NVIDIA's driver is unfree. Turn on \"Allow unfree packages\".".to_owned());
            }
        }

        Step::Apps => {
            if answers.catalog.is_empty() {
                fail(format!(
                    "The Rikka catalog could not be read from {PACKAGES_FLAKE}. Check the network \
                     and start the installer again."
                ));
            }
        }

        Step::Summary | Step::Install => problems.extend(validate(answers)),
    }

    problems
}

fn check_secret(value: &str, confirm: &str, what: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{what} is empty."));
    }
    if value != confirm {
        return Err(format!("{what} does not match."));
    }
    Ok(())
}

fn check_hostname(hostname: &str) -> Result<(), String> {
    if hostname.is_empty() {
        return Err("Hostname is empty.".to_owned());
    }
    if hostname.len() > 63 {
        return Err("Hostname is longer than 63 characters.".to_owned());
    }
    if hostname.contains('.') {
        return Err("Hostname cannot contain a dot.".to_owned());
    }
    if !hostname
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return Err("Hostname takes letters, digits and hyphens only.".to_owned());
    }
    if hostname.starts_with('-') || hostname.ends_with('-') {
        return Err("Hostname cannot start or end with a hyphen.".to_owned());
    }
    Ok(())
}

fn check_username(username: &str) -> Result<(), String> {
    if username.is_empty() {
        return Err("Username is empty.".to_owned());
    }
    if username.len() > 32 {
        return Err("Username is longer than 32 characters.".to_owned());
    }
    let mut chars = username.chars();
    let first = chars.next().unwrap_or_default();
    if !(first.is_ascii_lowercase() || first == '_') {
        return Err("Username must start with a lowercase letter or an underscore.".to_owned());
    }
    if !chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-') {
        return Err(
            "Username takes lowercase letters, digits, underscores and hyphens only.".to_owned(),
        );
    }
    if RESERVED_USERNAMES.contains(&username) {
        return Err(format!("{username} is a name the system uses."));
    }
    Ok(())
}
