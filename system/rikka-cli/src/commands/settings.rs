use std::io::{self, Write};

use cli_command::SettingsCommand;
use rikka_settings::{Constraint, Domain, Schema, Source, Value};
use serde::Serialize;

use crate::error::Result;
use crate::printer::{Printer, Report};
use crate::session_settings::SessionSettings;
use crate::table::Table;

pub(super) fn run(command: SettingsCommand, printer: Printer) -> Result<()> {
    let settings = SessionSettings::connect()?;
    match command {
        SettingsCommand::Domains => printer.report(&Domains(settings.domains()?)),
        SettingsCommand::List { domain } => {
            printer.report(&Listing::read(&settings, &domain.parse()?)?)
        }
        SettingsCommand::Describe { domain } => {
            printer.report(&SchemaReport(settings.schema(&domain.parse()?)?))
        }
        SettingsCommand::Get { domain, key } => {
            printer.report(&ValueReport(settings.get(&domain.parse()?, &key)?))
        }
        SettingsCommand::Set { domain, key, value } => {
            let domain: Domain = domain.parse()?;
            let value = settings.schema(&domain)?.parse(&key, &value)?;
            settings.set(&domain, &key, value)
        }
        SettingsCommand::Reset { domain, key } => settings.reset(&domain.parse()?, &key),
    }
}

#[derive(Serialize)]
#[serde(transparent)]
struct Domains(Vec<Domain>);

impl Report for Domains {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()> {
        for domain in &self.0 {
            writeln!(out, "{domain}")?;
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(transparent)]
struct Listing(Vec<Setting>);

#[derive(Serialize)]
struct Setting {
    key: String,
    value: Value,
    source: Source,
}

impl Listing {
    fn read(settings: &SessionSettings, domain: &Domain) -> Result<Listing> {
        let schema = settings.schema(domain)?;
        let mut values = settings.values(domain)?;
        let sources = settings.sources(domain)?;
        Ok(Listing(
            schema
                .keys()
                .iter()
                .map(|key| Setting {
                    key: key.name().to_owned(),
                    value: values
                        .remove(key.name())
                        .unwrap_or_else(|| key.default_value().clone()),
                    source: sources.get(key.name()).copied().unwrap_or(Source::Default),
                })
                .collect(),
        ))
    }
}

impl Report for Listing {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()> {
        let mut table = Table::new(["KEY", "SOURCE", "VALUE"]);
        for setting in &self.0 {
            table.row([
                setting.key.clone(),
                setting.source.to_string(),
                setting.value.to_string(),
            ]);
        }
        table.write(out)
    }
}

#[derive(Serialize)]
#[serde(transparent)]
struct SchemaReport(Schema);

impl Report for SchemaReport {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()> {
        if let Some(summary) = self.0.summary_text() {
            writeln!(out, "{summary}\n")?;
        }
        for key in self.0.keys() {
            writeln!(out, "{}", key.name())?;
            writeln!(
                out,
                "  {} (default {})",
                key.value_type(),
                key.default_value()
            )?;
            match key.constraint() {
                Constraint::Any => {}
                Constraint::Range { min, max } => writeln!(out, "  between {min} and {max}")?,
                Constraint::Choices(allowed) => {
                    let names: Vec<String> = allowed.iter().map(ToString::to_string).collect();
                    writeln!(out, "  one of {}", names.join(", "))?;
                }
            }
            if let Some(summary) = key.summary_text() {
                writeln!(out, "  {summary}")?;
            }
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(transparent)]
struct ValueReport(Value);

impl Report for ValueReport {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()> {
        writeln!(out, "{}", self.0)
    }
}
