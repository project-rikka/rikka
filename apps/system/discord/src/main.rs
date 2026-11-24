mod account;
mod banner;
mod brand;
mod chat;
mod conversation;
mod desk;
mod friends;
mod keys;
mod member_menu;
mod members;
mod notices;
mod pictures;
mod preferences;
mod profile;
mod rail;
mod screen;
mod settings;
mod sidebar;
mod signing;
mod space_sheet;
mod switcher;
mod voice;
mod widgets;

use std::sync::Arc;

use discord_client::Discord;
use discord_core::SavedAccount;
use discord_voice::Krisp;
use rikka_account_book::Accounts;
use rikka_app::prelude::*;
use rikka_dirs::Paths;
use rikka_ui_video::VideoStore;

use crate::chat::Chat;
use crate::preferences::Preferences;

fn window(
    discord: Arc<Discord>,
    krisp: Krisp,
    accounts: Accounts<SavedAccount>,
    preferences: Preferences,
) -> Window<Chat> {
    keys::bind(Window::new(
        move |sender| Chat::new(sender, discord, krisp, accounts, preferences),
        desk::view,
    ))
    .title(tr!("discord.title"))
    .size(1280.0, 800.0)
    .min_size(940.0, 560.0)
    .on_start(|chat: &mut Chat| chat.start())
    .on_focus(|chat: &mut Chat| chat.focused(true))
    .on_blur(|chat: &mut Chat| chat.focused(false))
    .caption(desk::caption_bar)
}

rikka_app::mimalloc!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = Paths::resolve()?;
    let accounts = Accounts::<SavedAccount>::open(paths.state_dir().join("discord"))?;
    let preferences = Preferences::open();
    let discord = Arc::new(Discord::open(paths.cache_dir().join("discord")));
    let krisp = Krisp::new(paths.cache_dir().join("discord").join("krisp"));
    App::new("rikka-discord")
        .instance(Instance::single())
        .plays_video(VideoStore::default())
        .i18n(rikka_i18n::embedded!())
        .window(window(discord, krisp, accounts, preferences))
        .run()?;
    rikka_web::wait_for_closed_runtimes();
    Ok(())
}
