use installer_core::{
    Answers, Channel, Disk, Filesystem, KEYBOARDS, Kernel, LOCALES, Offer, PartitionPlan, Start,
    Step, Strength,
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
        Step::Restore => restore(answers),
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
        Step::Restore => commit_restore(form, answers),
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
        Step::Restore => restore_detail(answers),
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

fn restore(answers: &Answers) -> Vec<Field> {
    let mut fields = vec![Field::choice(
        FieldId::Start,
        "Start from",
        Start::ALL
            .iter()
            .map(|start| Choice::new(start.label()))
            .collect(),
        index_of(Start::ALL, |start| *start == answers.start),
    )];
    match answers.start {
        Start::Fresh => {}
        Start::Backup => {
            fields.push(
                Field::text(
                    FieldId::BackupPath,
                    "Backup file",
                    answers.backup_path.clone(),
                )
                .placeholder("/run/media/usb/computer.rikka-backup"),
            );
            fields.push(Field::secret(
                FieldId::BackupPassphrase,
                "Passphrase",
                answers.backup_passphrase.clone(),
            ));
        }
        Start::Repository => fields.push(
            Field::text(
                FieldId::Repository,
                "Repository",
                answers.repository.clone(),
            )
            .placeholder("https://example.com/me/rikka.git"),
        ),
    }
    fields
}

fn commit_restore(form: &Form, answers: &mut Answers) {
    let start = picked(Start::ALL, form, FieldId::Start)
        .copied()
        .unwrap_or_default();
    let backup_path = match form.get(FieldId::BackupPath) {
        Some(_) => form.value(FieldId::BackupPath).to_owned(),
        None => answers.backup_path.clone(),
    };
    let backup_passphrase = match form.get(FieldId::BackupPassphrase) {
        Some(_) => form.secret(FieldId::BackupPassphrase),
        None => answers.backup_passphrase.clone(),
    };
    let repository = match form.get(FieldId::Repository) {
        Some(_) => form.value(FieldId::Repository).to_owned(),
        None => answers.repository.clone(),
    };
    let changed = start != answers.start
        || backup_path != answers.backup_path
        || backup_passphrase.as_str() != answers.backup_passphrase.as_str()
        || repository != answers.repository;
    if changed {
        answers.restored = None;
        answers.restore_problem = None;
    }
    answers.start = start;
    answers.backup_path = backup_path;
    answers.backup_passphrase = backup_passphrase;
    answers.repository = repository;
}

fn restore_detail(answers: &Answers) -> Vec<(String, Tone)> {
    let Some(restored) = &answers.restored else {
        return match answers.start {
            Start::Fresh => Vec::new(),
            Start::Backup | Start::Repository => vec![(
                "Press Enter to read it; the answers that follow start from what it holds."
                    .to_owned(),
                Tone::Plain,
            )],
        };
    };
    let mut lines = vec![(
        format!("Restores {} configuration modules.", restored.modules.len()),
        Tone::Plain,
    )];
    if restored.pending.is_some() {
        lines.push((
            format!(
                "{} changed settings and {} sign-ins finish in Settings → Backup & restore after the first login.",
                restored.preferences, restored.sign_ins
            ),
            Tone::Plain,
        ));
    }
    lines
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
