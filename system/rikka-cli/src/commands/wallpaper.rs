use std::io::{self, Write};

use cli_command::WallpaperCommand;
use rikka_session::wallpaper::{self, DisplayedWallpaper, WallpaperClient, WallpaperEntry};
use rikka_settings::session::appearance;
use rikka_settings::{Domain, Value};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::printer::{Printer, Report};
use crate::session::{CapabilitySocket, ReplyExt};
use crate::session_settings::SessionSettings;
use crate::table::Table;

pub(super) fn run(command: WallpaperCommand, printer: Printer) -> Result<()> {
    match command {
        WallpaperCommand::List => printer.report(&Library(list_wallpapers(&wallpaper_socket()?)?)),
        WallpaperCommand::Current => {
            let socket = wallpaper_socket()?;
            printer.report(&Current(
                WallpaperClient::new(socket.client()).showing().decode()?,
            ))
        }
        WallpaperCommand::Set { id } => {
            if !list_wallpapers(&wallpaper_socket()?)?
                .iter()
                .any(|listed| listed.id == id)
            {
                return Err(Error::NoSuchWallpaper(id));
            }
            set_wallpaper(id)
        }
        WallpaperCommand::Clear => set_wallpaper(appearance::UNSET.to_owned()),
        WallpaperCommand::Refresh => wallpaper_socket()?
            .client()
            .call(wallpaper::PROTOCOL.method(wallpaper::REFRESH))
            .finish(),
    }
}

fn wallpaper_socket() -> Result<CapabilitySocket> {
    CapabilitySocket::connect(wallpaper::CAPABILITY)
}

fn list_wallpapers(socket: &CapabilitySocket) -> Result<Vec<WallpaperEntry>> {
    WallpaperClient::new(socket.client()).list().decode()
}

fn set_wallpaper(id: String) -> Result<()> {
    let domain: Domain = appearance::DOMAIN.parse()?;
    SessionSettings::connect()?.set(&domain, appearance::WALLPAPER, Value::String(id))
}

#[derive(Serialize)]
#[serde(transparent)]
struct Library(Vec<WallpaperEntry>);

impl Report for Library {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()> {
        let mut table = Table::new(["ID", "NAME"]);
        for listed in &self.0 {
            table.row([listed.id.clone(), listed.name.clone()]);
        }
        table.write(out)
    }
}

#[derive(Serialize)]
#[serde(transparent)]
struct Current(Option<DisplayedWallpaper>);

impl Report for Current {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()> {
        match &self.0 {
            Some(shown) => writeln!(out, "{}", shown.id),
            None => Ok(()),
        }
    }
}
