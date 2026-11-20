use std::path::{Path, PathBuf};

use rikka_dirs::{UserDir, UserDirs, data_dirs};

const WRITTEN: &str = r#"# XDG_VIDEOS_DIR="$HOME/x"
 XDG_MUSIC_DIR = "$HOME/音楽" 
XDG_DOWNLOAD_DIR="$HOME/\$5 \`take\`"
XDG_DOCUMENTS_DIR="/srv/docs" # where they went
XDG_TEMPLATES_DIR=$HOME/unquoted
XDG_PUBLICSHARE_DIR="photos"
XDG_DESKTOP_DIR="/a"
XDG_DESKTOP_DIR="/b"
XDG_DESKTOP_DIR="relative"
XDG_PICTURES_DIR="$HOME/"
"#;

#[test]
fn user_dirs_follow_shell_and_spec() {
    let config = tempfile::tempdir().expect("a temporary configuration");
    std::fs::write(config.path().join("user-dirs.dirs"), WRITTEN).expect("written");
    unsafe {
        std::env::set_var("HOME", "/home/ada");
        std::env::set_var("XDG_CONFIG_HOME", config.path());
        std::env::set_var("XDG_DATA_HOME", "/home/ada/.local/share");
        std::env::set_var("XDG_DATA_DIRS", "/opt/share:/usr/share:/opt/share:relative");
    }

    let dirs = UserDirs::load();
    for (which, path) in [
        (UserDir::Music, Some("/home/ada/音楽")),
        (UserDir::Downloads, Some("/home/ada/$5 `take`")),
        (UserDir::Documents, Some("/srv/docs")),
        (UserDir::Templates, Some("/home/ada/Templates")),
        (UserDir::PublicShare, Some("/home/ada/Public")),
        (UserDir::Desktop, Some("/b")),
        (UserDir::Videos, Some("/home/ada/Videos")),
        (UserDir::Pictures, None),
    ] {
        assert_eq!(dirs.path(which), path.map(PathBuf::from), "{which:?}");
    }
    assert_eq!(
        dirs.wallpapers(),
        None,
        "a disabled pictures folder holds none"
    );
    assert_eq!(dirs.at(Path::new("/home/ada/音楽")), Some(UserDir::Music));
    assert_eq!(dirs.at(Path::new("/tmp")), None);

    assert_eq!(
        data_dirs(),
        [
            PathBuf::from("/home/ada/.local/share"),
            PathBuf::from("/opt/share"),
            PathBuf::from("/usr/share"),
        ]
    );
}
