mod appearance;
mod language;
mod logging;

use rikka_session::profile::Profile;
use rikka_tray::Tray;
use shell_apps::Apps;
use shell_assistant::AssistantPanel;
use shell_bar::Taskbar;
use shell_capture::Capture;
use shell_clipboard::ClipboardManager;
use shell_color_picker::ColorPicker;
use shell_desk::Desk;
use shell_desk_editor::DeskEditor;
use shell_emoji::Emoji;
use shell_feeds::{Daemons, Feeds};
use shell_ime::{InputMethod, InputMethods};
use shell_keyboard::OnScreenKeyboard;
use shell_kit::launch::LaunchService;
use shell_kit::{SessionLock, Shell};
use shell_launcher::Search;
use shell_lock::{KeyringPrompt, ScreenLock};
use shell_markup::{Editing, Markup};
use shell_notifications::{History, NotificationCenter, Quiet};
use shell_overview::Overview;
use shell_polkit::PolkitAgent;
use shell_power_saving::PowerSaving;
use shell_runtime::Result;
use shell_switcher::Switcher;
use shell_system_update::SystemUpdates;
use shell_translate::Translator;
use shell_trim::{InstantReplay, Trim};
use shell_wallpaper::{Backdrop, MaterialTheme};

const NAME: &str = "rikka-shell";

rikka_heap::mimalloc!();

fn main() -> Result<()> {
    rikka_heap::return_large_glibc_blocks();
    logging::install();

    shell_runtime::run(match Profile::from_environment() {
        Profile::Full => desktop(),
        Profile::Simple => backdrop(),
    })
}

fn desktop() -> Shell {
    let shell = Shell::new(NAME)
        .settings(appearance::schema())
        .settings(language::schema());

    let (shell, apps) = Apps::follow(shell);
    let daemons = Daemons::start();
    let (shell, feeds) = Feeds::follow(shell, daemons.clone());
    let session_lock = SessionLock::default();
    let lockscreen = ScreenLock::new(daemons.power().clone(), session_lock.clone());
    let power_saving = PowerSaving::new(
        daemons.power().clone(),
        daemons.battery().clone(),
        feeds.services.profiles().clone(),
    );
    let backdrop = Backdrop::beside_taskbar();
    let desk = Desk::new(shell.seam(), feeds.clone(), backdrop.shown_on());
    let desk_scene = desk.desk_scene();

    let quiet = Quiet::default();
    let history = History::default();
    let snip = shell_translate::snip();
    let editing = Editing::new();
    let methods = InputMethods::default();
    let capture = Capture::new(snip.clone(), editing.clone(), daemons.network().clone());

    language::follow(appearance::follow(shell))
        .service(LaunchService::new())
        .feature(backdrop)
        .feature(MaterialTheme::new())
        .feature(power_saving)
        .feature(Taskbar {
            recording: capture.recording(),
            daemons,
            feeds,
            tray: Tray::start(),
            apps: apps.clone(),
            quiet: quiet.clone(),
            history: history.clone(),
            methods: methods.clone(),
        })
        .feature(Search::new(apps.clone()))
        .feature(Overview::new(desk_scene.clone()))
        .feature(Switcher::new(apps.clone()))
        .feature(desk)
        .feature(DeskEditor::new(desk_scene))
        .feature(NotificationCenter::new(apps.clone(), quiet, history))
        .feature(SystemUpdates)
        .feature(ClipboardManager::new())
        .feature(Emoji::new())
        .feature(OnScreenKeyboard::new())
        .feature(InputMethod::new(apps.installed().clone(), methods))
        .feature(capture)
        .feature(Markup::new(editing))
        .feature(Trim::new())
        .feature(InstantReplay::new())
        .feature(Translator::new(snip))
        .feature(ColorPicker::new())
        .feature(PolkitAgent::new())
        .feature(lockscreen)
        .feature(KeyringPrompt::new(session_lock))
        .feature(AssistantPanel::new())
}

fn backdrop() -> Shell {
    tracing::info!("headless session, installing only the wallpaper");
    let shell = Shell::new(NAME).settings(appearance::schema());
    appearance::follow(shell).feature(Backdrop::alone())
}
