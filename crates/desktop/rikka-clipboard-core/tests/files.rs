use rikka_clipboard_core::{Files, Offer, files_in, offers_files};

fn read_back(offer: &Offer) -> Option<Files> {
    let offered: Vec<String> = offer.mime_types().map(str::to_owned).collect();
    files_in(&offered, |mime| offer.data(mime).map(|data| data.to_vec()))
}

#[test]
fn files_use_gnome_and_kde_formats() {
    let uris = ["file:///tmp/a%20b.png", "file:///tmp/notes"];
    for cut in [false, true] {
        assert_eq!(
            read_back(&Offer::files(&uris, cut)),
            Some(Files {
                uris: uris.map(str::to_owned).to_vec(),
                cut,
            })
        );
    }
}

#[test]
fn plain_uri_list_is_copy() {
    let offer = Offer::uris(&["file:///tmp/report.pdf"]);

    assert!(offers_files(
        &offer.mime_types().map(str::to_owned).collect::<Vec<_>>()
    ));
    assert_eq!(read_back(&offer).map(|files| files.cut), Some(false));
    assert_eq!(read_back(&Offer::text("plain")), None);
}
