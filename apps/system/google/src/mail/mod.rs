mod composer;
mod reader;
mod sidebar;
pub(crate) mod view;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::{self, StreamExt};
use google_mail::{
    Activity, Attachment, Error, INBOX, Label, Mailbox, Message, Pictures, Profile, Query, SPAM,
    STARRED, Summary, Thread, ThreadPage, UNREAD,
};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};

use crate::session::Session;
pub(crate) use composer::Composing;

const LISTING: &str = "google-mail-listing";
const READING: &str = "google-mail-reading";
const WATCH: &str = "google-mail-watch";
const WATCH_PERIOD: Duration = Duration::from_secs(60);
const COUNTED_LABELS: usize = 40;
const NEAR_END: usize = 6;
const PICTURES: &str = "pictures";
const ATTACHMENTS: &str = "attachments";
const REMOTE: [&str; 2] = ["https://", "http://"];
const INLINE: &str = "cid:";
const PICTURE_FETCHES: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Folder {
    Mailbox(Mailbox),
    Label { id: String, name: String },
    Search(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Listing {
    Loading,
    Ready,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Reading {
    Nothing,
    Loading,
    Ready(Arc<Thread>),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Notice {
    pub(crate) text: String,
    pub(crate) danger: bool,
}

#[derive(State)]
pub(crate) struct MailState {
    #[untracked]
    session: Option<Arc<Session>>,
    #[untracked]
    pictures: Option<Pictures>,
    labels: Tracked<Arc<Vec<Label>>>,
    folder: Tracked<Folder>,
    threads: Tracked<Arc<Vec<Summary>>>,
    next: Tracked<Option<String>>,
    listing: Tracked<Listing>,
    growing: Tracked<bool>,
    selected: Tracked<Option<String>>,
    reading: Tracked<Reading>,
    expanded: Tracked<HashSet<String>>,
    rendered: Tracked<Arc<HashMap<String, Rendered>>>,
    blocked: Tracked<usize>,
    remote: Tracked<bool>,
    #[untracked]
    wanted: HashSet<String>,
    #[untracked]
    search: TextDocument,
    seeking: Tracked<bool>,
    #[untracked]
    composing: Option<Composing>,
    composed: Revision,
    #[untracked]
    drafts: u64,
    sending: Tracked<bool>,
    notice: Tracked<Option<Notice>>,
    signed_out: Tracked<bool>,
    #[untracked]
    history_id: u64,
    #[untracked]
    era: u64,
    #[untracked]
    watched: bool,
}

impl Default for MailState {
    fn default() -> MailState {
        MailState {
            session: None,
            pictures: None,
            labels: Tracked::new(Arc::new(Vec::new())),
            folder: Tracked::new(Folder::Mailbox(Mailbox::Inbox)),
            threads: Tracked::new(Arc::new(Vec::new())),
            next: Tracked::new(None),
            listing: Tracked::new(Listing::Loading),
            growing: Tracked::new(false),
            selected: Tracked::new(None),
            reading: Tracked::new(Reading::Nothing),
            expanded: Tracked::new(HashSet::new()),
            rendered: Tracked::new(Arc::new(HashMap::new())),
            blocked: Tracked::new(0),
            remote: Tracked::new(false),
            wanted: HashSet::new(),
            search: TextDocument::new(""),
            seeking: Tracked::new(false),
            composing: None,
            composed: Revision::new(),
            drafts: 0,
            sending: Tracked::new(false),
            notice: Tracked::new(None),
            signed_out: Tracked::new(false),
            history_id: 0,
            era: 0,
            watched: false,
        }
    }
}

impl Folder {
    fn query(&self) -> Query {
        match self {
            Folder::Mailbox(mailbox) => Query {
                label: mailbox.label().map(str::to_owned),
                search: String::new(),
            },
            Folder::Label { id, .. } => Query {
                label: Some(id.clone()),
                search: String::new(),
            },
            Folder::Search(text) => Query {
                label: None,
                search: text.clone(),
            },
        }
    }
}

impl MailState {
    pub(crate) fn labels(&self) -> &[Label] {
        &self.labels
    }

    pub(crate) fn folder(&self) -> &Folder {
        &self.folder
    }

    pub(crate) fn threads(&self) -> &Arc<Vec<Summary>> {
        &self.threads
    }

    pub(crate) fn listing(&self) -> &Listing {
        &self.listing
    }

    pub(crate) fn is_growing(&self) -> bool {
        *self.growing
    }

    pub(crate) fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub(crate) fn selected_summary(&self) -> Option<&Summary> {
        let id = self.selected.as_deref()?;
        self.threads.iter().find(|thread| thread.id == id)
    }

    pub(crate) fn reading(&self) -> &Reading {
        &self.reading
    }

    pub(crate) fn is_expanded(&self, message: &str) -> bool {
        self.expanded.contains(message)
    }

    pub(crate) fn rendered(&self, message: &str) -> Option<Arc<str>> {
        self.rendered
            .get(message)
            .map(|rendered| Arc::clone(&rendered.text))
    }

    pub(crate) fn blocked_pictures(&self) -> usize {
        match *self.remote {
            true => 0,
            false => *self.blocked,
        }
    }

    pub(crate) fn pictures_folder(&self) -> Option<PathBuf> {
        self.pictures
            .as_ref()
            .map(|pictures| pictures.folder().to_owned())
    }

    pub(crate) fn search(&self) -> &TextDocument {
        &self.search
    }

    pub(crate) fn is_seeking(&self) -> bool {
        *self.seeking
    }

    pub(crate) fn composing(&self) -> Option<&Composing> {
        self.composed.read();
        self.composing.as_ref()
    }

    pub(crate) fn is_sending(&self) -> bool {
        *self.sending
    }

    pub(crate) fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    pub(crate) fn is_signed_out(&self) -> bool {
        *self.signed_out
    }

    pub(crate) fn account(&self) -> Option<&Session> {
        self.session.as_deref()
    }

    pub(crate) fn attach(&mut self, session: Arc<Session>) {
        self.detach();
        self.pictures = Some(Pictures::new(session.mail_cache.join(PICTURES)));
        let mail = session.mail.clone();
        self.session = Some(session);
        self.load(false);
        self.refresh_labels();
        spawn(
            async move { mail.profile().await },
            |mail: &mut MailState, profile: google_mail::Result<Profile>| {
                if let Ok(profile) = profile {
                    mail.history_id = mail.history_id.max(profile.history_id);
                }
                skip_view();
            },
        );
        every_keyed(WATCH, WATCH_PERIOD, MailState::check);
    }

    pub(crate) fn detach(&mut self) {
        cancel(LISTING);
        cancel(READING);
        cancel(WATCH);
        *self = MailState::default();
    }

    pub(crate) fn watch(&mut self, watched: bool) {
        self.watched = watched;
        if watched {
            self.check();
        }
    }

    pub(crate) fn go(&mut self, folder: Folder) {
        if *self.folder == folder {
            return;
        }
        self.folder.set(folder);
        self.selected.set(None);
        self.reading.set(Reading::Nothing);
        cancel(READING);
        self.load(false);
    }

    pub(crate) fn refresh(&mut self) {
        self.load(true);
        self.refresh_labels();
    }

    pub(crate) fn seek(&mut self) {
        self.seeking.set(true);
    }

    pub(crate) fn seek_ended(&mut self) {
        self.seeking.set(false);
    }
    pub(crate) fn sought(&mut self) {
        self.seeking.set(false);
        let text = self.search.text();
        match text.trim() {
            "" => self.go(Folder::Mailbox(Mailbox::Inbox)),
            text => self.go(Folder::Search(text.to_owned())),
        }
    }

    pub(crate) fn neared(&mut self, shown: std::ops::Range<usize>) {
        let near_end = shown.end + NEAR_END >= self.threads.len();
        match near_end && !*self.growing {
            true => self.grow(),
            false => skip_view(),
        }
    }

    pub(crate) fn select(&mut self, index: usize) {
        let Some(summary) = self.threads.get(index) else {
            return;
        };
        let id = summary.id.clone();
        if self.selected.as_deref() == Some(id.as_str()) {
            return;
        }
        let unread = summary.is_unread();
        self.selected.set(Some(id.clone()));
        self.read(id.clone());
        if unread {
            self.relabel(&id, &[], &[UNREAD]);
        }
    }

    pub(crate) fn close_thread(&mut self) {
        cancel(READING);
        self.selected.set(None);
        self.reading.set(Reading::Nothing);
    }

    pub(crate) fn toggle_message(&mut self, message: &str) {
        if !self.expanded.remove(message) {
            self.expanded.insert(message.to_owned());
            self.picture();
        }
    }

    pub(crate) fn show_remote_pictures(&mut self) {
        self.remote.set(true);
        self.picture();
    }

    pub(crate) fn open_link(&mut self, link: &str) {
        if let Err(error) = rikka_open::open(link) {
            self.notify(error.to_string(), true);
        } else {
            skip_view();
        }
    }

    pub(crate) fn archive(&mut self) {
        self.dismiss_thread(&[], &[INBOX]);
    }

    pub(crate) fn report_spam(&mut self) {
        self.dismiss_thread(&[SPAM], &[INBOX]);
    }

    pub(crate) fn mark_unread(&mut self) {
        if let Some(id) = (*self.selected).clone() {
            self.relabel(&id, &[UNREAD], &[]);
            self.close_thread();
        }
    }

    pub(crate) fn toggle_star(&mut self, id: &str) {
        let starred = self
            .threads
            .iter()
            .find(|thread| thread.id == id)
            .is_some_and(Summary::is_starred);
        match starred {
            true => self.relabel(id, &[], &[STARRED]),
            false => self.relabel(id, &[STARRED], &[]),
        }
    }

    pub(crate) fn trash(&mut self) {
        let (Some(session), Some(id)) = (&self.session, (*self.selected).clone()) else {
            return;
        };
        let restoring = *self.folder == Folder::Mailbox(Mailbox::Trash);
        let mail = session.mail.clone();
        self.forget(&id);
        spawn(
            async move {
                match restoring {
                    true => mail.untrash(&id).await,
                    false => mail.trash(&id).await,
                }
            },
            MailState::settled,
        );
    }

    pub(crate) fn open_attachment(&mut self, attachment: &Attachment) {
        let Some(session) = &self.session else {
            return;
        };
        let name = rikka_format::safe_file_name(&attachment.name)
            .unwrap_or_else(|| attachment.attachment_id.chars().take(16).collect());
        let folder = session
            .mail_cache
            .join(ATTACHMENTS)
            .join(&attachment.message_id);
        self.fetch_attachment(attachment, folder.join(name), true);
    }

    pub(crate) fn save_attachment(&mut self, attachment: &Attachment) {
        let attachment = attachment.clone();
        let parent = window_identifier();
        let name = attachment.name.clone();
        spawn_blocking(
            move || {
                Chooser::save(tr!("google.mail.attachment.save").to_string())
                    .parent(parent)
                    .current_name(name)
                    .run()
            },
            move |mail: &mut MailState, chosen: rikka_dialog::Result<ChooserResponse>| match chosen
            {
                Ok(ChooserResponse::Files(paths)) => {
                    if let Some(path) = paths.into_iter().next() {
                        mail.fetch_attachment(&attachment, path, false);
                    }
                }
                Ok(ChooserResponse::Cancelled) => skip_view(),
                Err(error) => mail.notify(error.to_string(), true),
            },
        );
    }

    pub(crate) fn dismiss_notice(&mut self) {
        self.notice.set(None);
    }

    pub(crate) fn notify(&mut self, text: String, danger: bool) {
        self.notice.set(Some(Notice { text, danger }));
    }

    pub(crate) fn failed(&mut self, error: &Error) {
        if error.is_signed_out() {
            self.signed_out.set(true);
        }
        self.notify(view::failure(error), true);
    }

    fn fetch_attachment(&mut self, attachment: &Attachment, destination: PathBuf, open: bool) {
        let Some(session) = &self.session else {
            return;
        };
        let mail = session.mail.clone();
        let attachment = attachment.clone();
        self.notify(
            tr!(
                "google.mail.attachment.fetching",
                name = attachment.name.as_str()
            )
            .to_string(),
            false,
        );
        spawn(
            async move {
                if !(open && destination.is_file()) {
                    let content = mail
                        .attachment(&attachment.message_id, &attachment.attachment_id)
                        .await
                        .map_err(|error| view::failure(&error))?;
                    if let Some(folder) = destination.parent() {
                        std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
                    }
                    let replace = rikka_fs::Replace::new(&destination);
                    match open {
                        true => replace.private(),
                        false => replace,
                    }
                    .write(&content)
                    .map_err(|error| error.to_string())?;
                }
                Ok(destination)
            },
            move |mail: &mut MailState, saved: Result<PathBuf, String>| match saved {
                Ok(path) if open => match rikka_open::open(&path.to_string_lossy()) {
                    Ok(()) => mail.notice.set(None),
                    Err(error) => mail.notify(error.to_string(), true),
                },
                Ok(path) => mail.notify(
                    tr!(
                        "google.mail.attachment.saved",
                        path = path.to_string_lossy().as_ref()
                    )
                    .to_string(),
                    false,
                ),
                Err(reason) => mail.notify(reason, true),
            },
        );
    }

    fn dismiss_thread(&mut self, add: &[&str], remove: &[&str]) {
        let Some(id) = (*self.selected).clone() else {
            return;
        };
        let leaves = match &*self.folder {
            Folder::Mailbox(mailbox) => {
                mailbox.label().is_some_and(|label| remove.contains(&label))
            }
            Folder::Label { .. } | Folder::Search(_) => false,
        };
        self.relabel(&id, add, remove);
        if leaves {
            self.forget(&id);
        }
    }

    fn forget(&mut self, id: &str) {
        let kept: Vec<Summary> = self
            .threads
            .iter()
            .filter(|thread| thread.id != id)
            .cloned()
            .collect();
        self.threads.set(Arc::new(kept));
        if self.selected.as_deref() == Some(id) {
            self.close_thread();
        }
    }

    fn relabel(&mut self, id: &str, add: &[&str], remove: &[&str]) {
        let Some(session) = &self.session else {
            return;
        };
        let mut threads = (**self.threads).clone();
        if let Some(thread) = threads.iter_mut().find(|thread| thread.id == id) {
            thread.relabel(add, remove);
        }
        self.threads.set(Arc::new(threads));
        let mail = session.mail.clone();
        let id = id.to_owned();
        let add: Vec<String> = add.iter().map(|label| (*label).to_owned()).collect();
        let remove: Vec<String> = remove.iter().map(|label| (*label).to_owned()).collect();
        spawn(
            async move {
                let add: Vec<&str> = add.iter().map(String::as_str).collect();
                let remove: Vec<&str> = remove.iter().map(String::as_str).collect();
                mail.relabel(&id, &add, &remove).await
            },
            MailState::settled,
        );
    }

    fn settled(&mut self, settled: google_mail::Result<()>) {
        match settled {
            Ok(()) => {
                self.refresh_labels();
                skip_view();
            }
            Err(error) => {
                self.failed(&error);
                self.load(true);
            }
        }
    }

    fn load(&mut self, quietly: bool) {
        let Some(session) = &self.session else {
            return;
        };
        self.era += 1;
        if !quietly {
            self.threads.set(Arc::new(Vec::new()));
            self.next.set(None);
            self.listing.set(Listing::Loading);
        }
        let mail = session.mail.clone();
        let query = self.folder.query();
        let era = self.era;
        spawn_keyed(
            LISTING,
            async move { mail.threads(&query, None).await },
            move |mail: &mut MailState, page: google_mail::Result<ThreadPage>| {
                if mail.era != era {
                    return skip_view();
                }
                match page {
                    Ok(page) => mail.listed(page, quietly),
                    Err(error) if mail.threads.is_empty() => {
                        if error.is_signed_out() {
                            mail.signed_out.set(true);
                        }
                        mail.listing.set(Listing::Failed(view::failure(&error)));
                    }
                    Err(error) => mail.failed(&error),
                }
            },
        );
    }

    fn listed(&mut self, page: ThreadPage, quietly: bool) {
        self.history_id = page
            .threads
            .iter()
            .map(|thread| thread.history_id)
            .fold(self.history_id, u64::max);
        let oldest = page
            .threads
            .last()
            .map_or(i64::MIN, |thread| thread.date_ms);
        let mut threads = page.threads;
        if quietly && self.next.is_some() {
            let fresh: HashSet<&str> = threads.iter().map(|thread| thread.id.as_str()).collect();
            let older: Vec<Summary> = self
                .threads
                .iter()
                .filter(|thread| thread.date_ms < oldest && !fresh.contains(thread.id.as_str()))
                .cloned()
                .collect();
            threads.extend(older);
        } else {
            self.next.set(page.next);
        }
        self.threads.set(Arc::new(threads));
        self.listing.set(Listing::Ready);
    }

    fn grow(&mut self) {
        let (Some(session), Some(next)) = (&self.session, (*self.next).clone()) else {
            return skip_view();
        };
        self.growing.set(true);
        let mail = session.mail.clone();
        let query = self.folder.query();
        let era = self.era;
        spawn(
            async move { mail.threads(&query, Some(&next)).await },
            move |mail: &mut MailState, page: google_mail::Result<ThreadPage>| {
                if mail.era != era {
                    return skip_view();
                }
                mail.growing.set(false);
                match page {
                    Ok(page) => {
                        let mut threads = (**mail.threads).clone();
                        let known: HashSet<String> =
                            threads.iter().map(|thread| thread.id.clone()).collect();
                        threads.extend(
                            page.threads
                                .into_iter()
                                .filter(|thread| !known.contains(&thread.id)),
                        );
                        mail.threads.set(Arc::new(threads));
                        mail.next.set(page.next);
                    }
                    Err(error) => {
                        mail.next.set(None);
                        mail.failed(&error);
                    }
                }
            },
        );
    }

    fn refresh_labels(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        let mail = session.mail.clone();
        spawn(
            async move {
                let mut labels = mail.labels().await?;
                let counted = |label: &Label| {
                    label.is_listed()
                        || Mailbox::ALL
                            .iter()
                            .any(|mailbox| mailbox.counts_unread() && mailbox.id() == label.id)
                };
                let indices: Vec<usize> = labels
                    .iter()
                    .enumerate()
                    .filter(|(_, label)| counted(label))
                    .map(|(index, _)| index)
                    .take(COUNTED_LABELS)
                    .collect();
                let ids: Vec<String> = indices
                    .iter()
                    .map(|&index| labels[index].id.clone())
                    .collect();
                let detailed = mail.detailed_labels(&ids).await?;
                for (index, label) in indices.into_iter().zip(detailed) {
                    labels[index] = label;
                }
                Ok(labels)
            },
            |mail: &mut MailState, labels: google_mail::Result<Vec<Label>>| match labels {
                Ok(mut labels) => {
                    labels
                        .sort_by(|left, right| rikka_format::natural_cmp(&left.name, &right.name));
                    mail.labels.set(Arc::new(labels));
                }
                Err(error) => {
                    tracing::debug!("failed to read the mail labels: {error}");
                    skip_view();
                }
            },
        );
    }

    fn check(&mut self) {
        skip_view();
        let (Some(session), true) = (&self.session, self.watched) else {
            return;
        };
        if self.history_id == 0 {
            return;
        }
        let mail = session.mail.clone();
        let since = self.history_id;
        spawn_keyed(
            WATCH,
            async move { mail.activity(since).await },
            |mail: &mut MailState, activity: google_mail::Result<Activity>| match activity {
                Ok(activity) => {
                    mail.history_id = mail.history_id.max(activity.history_id);
                    match activity.threads.is_empty() {
                        true => skip_view(),
                        false => mail.refreshed(&activity.threads),
                    }
                }
                Err(Error::HistoryExpired) => {
                    mail.history_id = 0;
                    mail.refresh();
                }
                Err(error) => {
                    tracing::debug!("failed to check for new mail: {error}");
                    skip_view();
                }
            },
        );
    }

    fn refreshed(&mut self, touched: &[String]) {
        self.load(true);
        self.refresh_labels();
        let open = self
            .selected
            .as_ref()
            .filter(|open| touched.contains(open))
            .cloned();
        if let Some(open) = open {
            self.read(open);
        }
    }

    fn read(&mut self, id: String) {
        let (Some(session), Some(pictures)) = (&self.session, &self.pictures) else {
            return;
        };
        let reopened = matches!(&*self.reading, Reading::Ready(thread) if thread.id == id);
        if !reopened {
            self.reading.set(Reading::Loading);
            self.expanded.set(HashSet::new());
            self.rendered.set(Arc::new(HashMap::new()));
            self.blocked.set(0);
            self.remote.set(false);
            self.wanted.clear();
        }
        let mail = session.mail.clone();
        let pictures = pictures.clone();
        spawn_keyed(
            READING,
            async move {
                let thread = mail.thread(&id).await?;
                let rendered = Rendered::thread(&thread, &pictures);
                Ok((thread, rendered))
            },
            |mail: &mut MailState,
             opened: google_mail::Result<(Thread, HashMap<String, Rendered>)>| {
                match opened {
                    Ok((thread, rendered)) => mail.opened(thread, rendered),
                    Err(error) => {
                        if error.is_signed_out() {
                            mail.signed_out.set(true);
                        }
                        mail.reading.set(Reading::Failed(view::failure(&error)));
                    }
                }
            },
        );
    }

    fn opened(&mut self, thread: Thread, rendered: HashMap<String, Rendered>) {
        if self.selected.as_deref() != Some(thread.id.as_str()) {
            return skip_view();
        }
        let mut expanded = (*self.expanded).clone();
        let unread: Vec<&Message> = thread
            .messages
            .iter()
            .filter(|message| message.has(UNREAD))
            .collect();
        expanded.extend(unread.iter().map(|message| message.id.clone()));
        if let Some(last) = thread.messages.last() {
            expanded.insert(last.id.clone());
        }
        self.expanded.set(expanded);
        self.rendered.set(Arc::new(rendered));
        self.reading.set(Reading::Ready(Arc::new(thread)));
        self.picture();
    }

    fn picture(&mut self) {
        let (Some(session), Some(pictures), Reading::Ready(thread)) =
            (&self.session, &self.pictures, &*self.reading)
        else {
            return;
        };
        let mut blocked = 0;
        let mut missing: Vec<(String, String, Option<Attachment>)> = Vec::new();
        for message in &thread.messages {
            if !self.expanded.contains(&message.id) {
                continue;
            }
            let Some(rendered) = self.rendered.get(&message.id) else {
                continue;
            };
            for source in &rendered.missing {
                let remote = REMOTE.iter().any(|scheme| source.starts_with(scheme));
                if remote && !*self.remote {
                    blocked += 1;
                    continue;
                }
                let attachment = source
                    .starts_with(INLINE)
                    .then(|| message.inline_picture(source).cloned())
                    .flatten();
                let key = key(message, source);
                if (remote || attachment.is_some()) && self.wanted.insert(key.clone()) {
                    missing.push((key, source.clone(), attachment));
                }
            }
        }
        self.blocked.set(blocked);
        if missing.is_empty() {
            return;
        }
        let mail = session.mail.clone();
        let http = session.http.clone();
        let pictures = pictures.clone();
        let thread = Arc::clone(thread);
        spawn(
            async move {
                stream::iter(missing)
                    .for_each_concurrent(PICTURE_FETCHES, |(key, source, attachment)| {
                        let (mail, http, pictures) = (&mail, &http, &pictures);
                        async move {
                            let picture = match attachment {
                                Some(attachment) => mail
                                    .attachment(&attachment.message_id, &attachment.attachment_id)
                                    .await
                                    .map_err(|error| error.to_string()),
                                None => http
                                    .get_bytes(source)
                                    .await
                                    .map(|bytes| bytes.to_vec())
                                    .map_err(|error| error.to_string()),
                            };
                            let kept = picture.and_then(|picture| {
                                pictures
                                    .keep(&key, &picture)
                                    .map_err(|error| error.to_string())
                            });
                            if let Err(error) = kept {
                                tracing::debug!("failed to keep a mail picture: {error}");
                            }
                        }
                    })
                    .await;
                let rendered = Rendered::thread(&thread, &pictures);
                (thread.id.clone(), rendered)
            },
            |mail: &mut MailState, (thread_id, rendered): (String, HashMap<String, Rendered>)| {
                mail.pictured(&thread_id, rendered);
            },
        );
    }

    fn pictured(&mut self, thread_id: &str, rendered: HashMap<String, Rendered>) {
        match &*self.reading {
            Reading::Ready(thread) if thread.id == thread_id => {
                self.rendered.set(Arc::new(rendered));
            }
            _ => skip_view(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Rendered {
    text: Arc<str>,
    missing: Vec<String>,
}

impl Rendered {
    fn thread(thread: &Thread, pictures: &Pictures) -> HashMap<String, Rendered> {
        thread
            .messages
            .iter()
            .map(|message| (message.id.clone(), Rendered::message(message, pictures)))
            .collect()
    }

    fn message(message: &Message, pictures: &Pictures) -> Rendered {
        let mut missing = Vec::new();
        let text = message.readable(|source| {
            let kept = pictures.kept(&key(message, source));
            if kept.is_none() {
                missing.push(source.to_owned());
            }
            kept
        });
        Rendered {
            text: Arc::from(text),
            missing,
        }
    }
}

fn key(message: &Message, source: &str) -> String {
    match source.starts_with(INLINE) {
        true => format!("{}:{source}", message.id),
        false => source.to_owned(),
    }
}
