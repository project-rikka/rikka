use installer_core::{
    Answers, Channel, Disk, Filesystem, KEYBOARDS, Kernel, LOCALES, Offer, PartitionPlan, Step,
    Strength,
};

use crate::form::{Choice, Field, FieldId, Form, Tone};

fn index_of<T>(options: &[T], predicate: impl FnMut(&T) -> bool) -> usize {
    options.iter().position(predicate).unwrap_or_default()
}

fn picked<'a, T>(options: &'a [T], form: &Form, id: FieldId) -> Option<&'a T> {
    options.get(form.index(id))
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Context<'a> {
    pub(crate) answers: &'a Answers,
    pub(crate) disks: &'a [Disk],
    pub(crate) timezones: &'a [String],
}

#[must_use]
pub(crate) fn build(step: Step, cx: Context<'_>) -> Form {
    let answers = cx.answers;
    let fields = match step {
        Step::Language => language(answers),
        Step::Keyboard => keyboard(answers),
        Step::Timezone => timezone(answers, cx.timezones),
        Step::Disk => disk(answers, cx.disks),
        Step::Account => account(answers),
        Step::System => system(answers),
        Step::Apps => apps(answers),
        Step::Summary | Step::Install => Vec::new(),
    };
    Form::new(fields)
}

pub(crate) fn commit(
    step: Step,
    form: &Form,
    answers: &mut Answers,
    disks: &[Disk],
    timezones: &[String],
) {
    match step {
        Step::Language => {
            if let Some(locale) = picked(LOCALES, form, FieldId::Locale) {
                answers.locale = locale;
            }
            answers.english_messages =
                form.on_or(FieldId::EnglishMessages, answers.english_messages);
        }
        Step::Keyboard => {
            if let Some(keyboard) = picked(KEYBOARDS, form, FieldId::Keyboard) {
                answers.keyboard = keyboard;
            }
        }
        Step::Timezone => {
            if let Some(zone) = picked(timezones, form, FieldId::Timezone) {
                answers.timezone.clone_from(zone);
            }
        }
        Step::Disk => {
            answers.disk = picked(disks, form, FieldId::Disk).cloned();
            if let Some(filesystem) = picked(Filesystem::ALL, form, FieldId::Filesystem) {
                answers.filesystem = *filesystem;
            }
            let chosen = picked(&answers.swap_choices(), form, FieldId::Swap).copied();

            answers.encrypt = form.is_checked(FieldId::Encrypt);
            answers.swap = chosen
                .filter(|swap| answers.swap_choices().contains(swap))
                .unwrap_or_default();
            answers.passphrase = form.secret(FieldId::Passphrase);
            answers.passphrase_confirm = form.secret(FieldId::PassphraseConfirm);
        }
        Step::Account => {
            answers.hostname = form.value(FieldId::Hostname).to_owned();
            answers.username = form.value(FieldId::Username).to_owned();
            answers.full_name = form.value(FieldId::FullName).to_owned();
            answers.password = form.secret(FieldId::Password);
            answers.password_confirm = form.secret(FieldId::PasswordConfirm);
            answers.set_root_password = form.is_checked(FieldId::SetRootPassword);
            answers.root_password = form.secret(FieldId::RootPassword);
            answers.root_password_confirm = form.secret(FieldId::RootPasswordConfirm);
        }
        Step::System => {
            if let Some(channel) = picked(Channel::ALL, form, FieldId::Channel) {
                answers.channel = *channel;
            }
            if let Some(kernel) = picked(Kernel::ALL, form, FieldId::Kernel) {
                answers.kernel = *kernel;
            }
            answers.allow_unfree = form.is_checked(FieldId::AllowUnfree);
            answers.openssh = form.is_checked(FieldId::OpenSsh);
            answers.ssh_password_auth =
                form.on_or(FieldId::SshPasswordAuth, answers.ssh_password_auth);
            answers.nvidia = form.on_or(FieldId::Nvidia, answers.nvidia);
            answers.sudo_rs = form.is_checked(FieldId::SudoRs);
        }
        Step::Apps => {
            let ids: Vec<String> = answers
                .catalog
                .optional()
                .map(|component| component.id.clone())
                .collect();
            for (index, id) in ids.iter().enumerate() {
                answers.choose_app(id, form.is_checked(FieldId::App(index)));
            }
        }
        Step::Summary | Step::Install => {}
    }
}

#[must_use]
pub(crate) fn detail(step: Step, answers: &Answers) -> Vec<(String, Tone)> {
    match step {
        Step::Disk => disk_detail(answers),

        Step::Account => {
            let password = answers.password.as_str();
            if password.is_empty() {
                return Vec::new();
            }
            let strength = Strength::measure(password);
            let tone = match strength {
                Strength::Weak => Tone::Warning,
                _ => Tone::Plain,
            };
            vec![(format!("Password: {}", strength.label()), tone)]
        }

        Step::System => answers
            .graphics()
            .into_iter()
            .map(|line| (line, Tone::Plain))
            .collect(),

        Step::Apps => {
            let required: Vec<&str> = answers
                .catalog
                .components()
                .iter()
                .filter(|component| component.offer == Offer::Required)
                .map(|component| component.name.as_str())
                .collect();
            match required.is_empty() {
                true => Vec::new(),
                false => vec![(
                    format!("Always installed: {}", required.join(", ")),
                    Tone::Plain,
                )],
            }
        }

        Step::Language | Step::Keyboard | Step::Timezone | Step::Summary | Step::Install => {
            Vec::new()
        }
    }
}

fn disk_detail(answers: &Answers) -> Vec<(String, Tone)> {
    let Some(disk) = &answers.disk else {
        return Vec::new();
    };

    let mut lines = match PartitionPlan::build(answers) {
        Ok(plan) => plan
            .describe()
            .into_iter()
            .map(|line| (line, Tone::Plain))
            .collect(),
        Err(error) => vec![(error.to_string(), Tone::Danger)],
    };
    lines.push((
        format!("Everything on {} will be erased.", disk.name),
        Tone::Danger,
    ));
    lines
}

fn language(answers: &Answers) -> Vec<Field> {
    let selected = index_of(LOCALES, |locale| locale == answers.locale);
    let options = LOCALES
        .iter()
        .map(|locale| Choice::with_note(locale.label, locale.name))
        .collect();

    let mut fields = vec![Field::picker(FieldId::Locale, "Locale", options, selected)];
    if !answers.locale.english {
        fields.push(Field::toggle(
            FieldId::EnglishMessages,
            "English messages",
            answers.english_messages,
        ));
    }
    fields
}

fn keyboard(answers: &Answers) -> Vec<Field> {
    let selected = index_of(KEYBOARDS, |keyboard| keyboard == answers.keyboard);
    let options = KEYBOARDS
        .iter()
        .map(|keyboard| {
            Choice::with_note(
                keyboard.label,
                format!("{} / {}", keyboard.keymap, keyboard.layout),
            )
        })
        .collect();

    vec![Field::picker(
        FieldId::Keyboard,
        "Keyboard",
        options,
        selected,
    )]
}

fn timezone(answers: &Answers, timezones: &[String]) -> Vec<Field> {
    let selected = index_of(timezones, |zone| *zone == answers.timezone);
    let options = timezones
        .iter()
        .map(|zone| Choice::new(zone.as_str()))
        .collect();

    vec![Field::picker(
        FieldId::Timezone,
        "Time zone",
        options,
        selected,
    )]
}

fn disk(answers: &Answers, disks: &[Disk]) -> Vec<Field> {
    let selected = answers
        .disk
        .as_ref()
        .and_then(|chosen| disks.iter().position(|disk| disk.path == chosen.path))
        .unwrap_or_default();

    let filesystems = Filesystem::ALL
        .iter()
        .map(|filesystem| Choice::new(filesystem.label()))
        .collect();
    let swaps = answers.swap_choices();
    let swap_index = index_of(&swaps, |swap| *swap == answers.swap);

    let mut fields = vec![
        Field::disks(FieldId::Disk, "Target", disks.to_vec(), selected),
        Field::choice(
            FieldId::Filesystem,
            "Filesystem",
            filesystems,
            index_of(Filesystem::ALL, |filesystem| {
                *filesystem == answers.filesystem
            }),
        ),
        Field::choice(
            FieldId::Swap,
            "Swap",
            swaps.iter().map(|swap| Choice::new(swap.label())).collect(),
            swap_index,
        ),
        Field::toggle(FieldId::Encrypt, "Encrypt with LUKS2", answers.encrypt),
    ];

    if answers.encrypt {
        fields.push(Field::secret(
            FieldId::Passphrase,
            "Passphrase",
            answers.passphrase.clone(),
        ));
        fields.push(Field::secret(
            FieldId::PassphraseConfirm,
            "Confirm",
            answers.passphrase_confirm.clone(),
        ));
    }

    fields
}

fn account(answers: &Answers) -> Vec<Field> {
    let mut fields = vec![
        Field::text(FieldId::Hostname, "Hostname", answers.hostname.clone()),
        Field::text(FieldId::Username, "Username", answers.username.clone()).placeholder("user"),
        Field::text(FieldId::FullName, "Full name", answers.full_name.clone())
            .placeholder("optional"),
        Field::secret(FieldId::Password, "Password", answers.password.clone()),
        Field::secret(
            FieldId::PasswordConfirm,
            "Confirm",
            answers.password_confirm.clone(),
        ),
        Field::toggle(
            FieldId::SetRootPassword,
            "Set a root password",
            answers.set_root_password,
        ),
    ];

    if answers.set_root_password {
        fields.push(Field::secret(
            FieldId::RootPassword,
            "Root password",
            answers.root_password.clone(),
        ));
        fields.push(Field::secret(
            FieldId::RootPasswordConfirm,
            "Confirm",
            answers.root_password_confirm.clone(),
        ));
    }

    fields
}

fn system(answers: &Answers) -> Vec<Field> {
    let channels = Channel::ALL
        .iter()
        .map(|channel| Choice::new(channel.label(&answers.release)))
        .collect();

    let mut fields = vec![
        Field::choice(
            FieldId::Channel,
            "nixpkgs",
            channels,
            index_of(Channel::ALL, |channel| *channel == answers.channel),
        ),
        Field::choice(
            FieldId::Kernel,
            "Kernel",
            Kernel::ALL
                .iter()
                .map(|kernel| Choice::new(kernel.label()))
                .collect(),
            index_of(Kernel::ALL, |kernel| *kernel == answers.kernel),
        ),
        Field::toggle(FieldId::OpenSsh, "OpenSSH server", answers.openssh),
    ];

    if answers.openssh {
        fields.push(Field::toggle(
            FieldId::SshPasswordAuth,
            "SSH password authentication",
            answers.ssh_password_auth,
        ));
    }

    if answers.nvidia_offer().is_some() {
        fields.push(Field::toggle(
            FieldId::Nvidia,
            "NVIDIA driver",
            answers.nvidia,
        ));
    }
    fields.push(Field::toggle(
        FieldId::AllowUnfree,
        "Allow unfree packages",
        answers.allow_unfree,
    ));
    fields.push(Field::toggle(
        FieldId::SudoRs,
        "Use sudo-rs",
        answers.sudo_rs,
    ));
    fields
}

fn apps(answers: &Answers) -> Vec<Field> {
    answers
        .catalog
        .optional()
        .enumerate()
        .map(|(index, component)| {
            Field::toggle(
                FieldId::App(index),
                format!("{} — {}", component.name, component.summary),
                answers.apps.contains(&component.id),
            )
        })
        .collect()
}
