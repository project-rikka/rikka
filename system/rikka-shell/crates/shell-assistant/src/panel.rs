use std::path::PathBuf;

use rikka_chat::{
    Attached, Builtin, Draft, DraftChange, Drafts, Gathered, Models, OFFER_DWELL, Offer, Offering,
    Offers, Picking, QuestionForm, QuestionQueue, Servers, Transcript, argument_offers, gather,
    invoked, mentioned, needs_mcp, prompt_offers, read_mentioned, resource_offers,
};
use rikka_llm::Effort;
use rikka_llm_accounts::Selection;
use rikka_seam::{Request, SeamClient};
use rikka_thread::{Ask, Attachment};
use rikka_ui::search::Action;
use rikka_ui::{State, TextDocument, after, skip_view, spawn, spawn_blocking, stream};

use crate::assistance::{Assistance, LayoutChange};
use crate::chat::{Calls, ShellChat};
use crate::service::{CAPABILITY, CLOSE};
use crate::shown::Shown;
use crate::{fitting, mention, turn_context};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tab {
    #[default]
    Conversation,
    Threads,
    Servers,
}

#[derive(State)]
pub(crate) struct Panel {
    seam: SeamClient,
    assistance: Assistance,
    home: Option<PathBuf>,
    shown: Shown,
    #[state]
    transcript: Transcript,
    #[state]
    offers: Offers,
    #[state]
    servers: Servers,
    draft: TextDocument,
    attached: Vec<Attachment>,
    drafts: Drafts,
    sending: bool,
    tab: Tab,
    asks: QuestionQueue,
    models: Models,
    picking: Picking,
}

impl Panel {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, assistance: Assistance) -> Panel {
        Panel {
            shown: assistance.shown(),
            transcript: Transcript::new(Calls::new(assistance.apps().clone())),
            servers: Servers::new(assistance.hub().clone()),
            seam,
            assistance,
            home: fitting::home(),
            offers: Offers::default(),
            draft: TextDocument::default(),
            attached: Vec::new(),
            drafts: Drafts::default(),
            sending: false,
            tab: Tab::default(),
            asks: QuestionQueue::default(),
            models: Models::default(),
            picking: Picking::default(),
        }
    }

    pub(crate) fn start(&mut self) {
        self.refresh();
        stream(
            self.assistance.beats(),
            |panel: &mut Panel, stirred: LayoutChange| {
                panel.stirred(stirred);
            },
        );
    }

    pub(crate) fn shown(&self) -> &Shown {
        &self.shown
    }

    pub(super) fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    pub(crate) fn draft(&self) -> &TextDocument {
        &self.draft
    }

    pub(super) fn attached(&self) -> &[Attachment] {
        &self.attached
    }

    pub(crate) fn offering(&self) -> Option<&Offering> {
        self.offers.offering()
    }

    pub(super) fn is_sending(&self) -> bool {
        self.sending
    }

    pub(crate) fn tab(&self) -> Tab {
        self.tab
    }

    pub(crate) fn asking(&self) -> Option<&QuestionForm> {
        self.asks.open()
    }

    pub(crate) fn models(&self) -> &Models {
        &self.models
    }

    pub(crate) fn picking(&self) -> Picking {
        self.picking
    }

    pub(super) fn servers(&self) -> &Servers {
        &self.servers
    }

    pub(crate) fn show(&mut self, tab: Tab) {
        if tab == Tab::Servers && self.tab != Tab::Servers {
            self.servers.follow();
        }
        self.tab = tab;
    }

    fn stirred(&mut self, stirred: LayoutChange) {
        let Panel {
            assistance,
            transcript,
            ..
        } = self;
        let streamed = stirred == LayoutChange::Grew
            && assistance
                .answering(|answer| transcript.stream(Some(answer)))
                .unwrap_or_default();
        if streamed {
            return skip_view();
        }
        self.refresh();
    }

    fn refresh(&mut self) {
        let selection = self.shown.selection.clone();
        let was = self.shown.thread.id().to_owned();
        self.shown = self.assistance.shown();
        if self.shown.thread.id() != was {
            self.switched(&was);
        }
        self.transcript
            .refresh(Some(&self.shown.thread), self.shown.answering.as_ref());
        self.asks.track(&self.shown.surveys);
        if let Some(kit) = self.assistance.take_summoned() {
            self.point_at(&kit);
        }
        if self.reload_models(selection.as_ref()) {
            self.models = self.assistance.models(self.shown.selection.as_ref());
        }
    }

    fn switched(&mut self, was: &str) {
        self.drafts.stash(
            was,
            Draft::new(
                std::mem::take(&mut self.draft),
                std::mem::take(&mut self.attached),
            ),
        );
        if let Some(draft) = self.drafts.restore(self.shown.thread.id()) {
            self.draft = draft.text;
            self.attached = draft.attached;
        }
        self.transcript.forget();
        self.offers.dismiss();
        self.picking = Picking::new(false);
        self.tab = Tab::Conversation;
    }

    fn reload_models(&self, shown: Option<&Selection>) -> bool {
        !self.shown.is_answering()
            || self.models.is_empty()
            || shown != self.shown.selection.as_ref()
    }

    fn point_at(&mut self, kit: &str) {
        let message = self.draft.text();
        if !turn_context::points_at(&message, kit) {
            self.draft.set_text(format!("@{kit} {message}"));
        }
        let end = self.draft.text().len();
        self.draft.place_caret(end, false);
        self.offers.dismiss();
        self.tab = Tab::Conversation;
    }

    pub(crate) fn send(&mut self) {
        if self.shown.is_answering() || self.sending || !self.shown.is_ready() {
            return skip_view();
        }
        self.offers.dismiss();
        self.tab = Tab::Conversation;

        let text = self.draft.text();
        if let Some(invoked) = invoked(&text)
            && let Some(Builtin::Compact) = Builtin::named(invoked.name())
        {
            if self.assistance.compact(invoked.arguments()) {
                self.draft.set_text("");
            }
            return self.refresh();
        }
        if text.trim().is_empty() && self.attached.is_empty() {
            return skip_view();
        }

        let ask = Ask::new(text.trim());
        let hub = self.assistance.hub().clone();
        if needs_mcp(&hub, &ask) {
            self.sending = true;
            spawn(
                async move { gather::<ShellChat>(&hub, ask).await },
                |panel: &mut Panel, gathered: Result<Gathered, String>| panel.fetched(gathered),
            );
            return;
        }
        let mentions = mentioned(ask.text());
        self.collect(ask, Attached::default(), mentions);
    }

    fn fetched(&mut self, gathered: Result<Gathered, String>) {
        match gathered {
            Ok(Gathered {
                ask,
                attached,
                mentions,
            }) => self.collect(ask, attached, mentions),
            Err(reason) => {
                self.sending = false;
                self.assistance.show_error(reason);
                self.refresh();
            }
        }
    }

    fn collect(&mut self, ask: Ask, attached: Attached, mentions: Vec<String>) {
        let running = self.assistance.apps().running();
        let paths: Vec<String> = mentions
            .into_iter()
            .filter(|named| !turn_context::names_app(named, &running))
            .collect();
        let Some(home) = self.home.clone().filter(|_| !paths.is_empty()) else {
            return self.ask(ask, attached);
        };
        self.sending = true;
        spawn_blocking(
            move || {
                let read = read_mentioned::<ShellChat>(&home, &paths, mention::listed);
                (ask, attached.join(read))
            },
            |panel: &mut Panel, (ask, attached): (Ask, Attached)| panel.ask(ask, attached),
        );
    }

    fn ask(&mut self, ask: Ask, mentioned: Attached) {
        self.sending = false;
        let mut attached = self.attached.clone();
        attached.extend(mentioned.attachments);
        if self.assistance.send_message(ask.with_attachments(attached)) {
            self.draft.set_text("");
            self.attached.clear();
        }
        if let Some(reason) = mentioned.refused {
            self.assistance.show_error(reason);
        }
        self.refresh();
    }

    pub(crate) fn stop(&mut self) {
        self.assistance.stop();
        self.refresh();
    }

    pub(crate) fn begin(&mut self) {
        self.tab = Tab::Conversation;
        self.assistance.begin_thread();
        self.refresh();
    }

    pub(crate) fn open(&mut self, id: &str) {
        self.tab = Tab::Conversation;
        self.assistance.open_thread(id);
        self.refresh();
    }

    pub(crate) fn discard(&mut self, id: &str) {
        let open = self.shown.thread.id() == id;
        if self.assistance.discard_thread(id) && open {
            self.draft.set_text("");
            self.attached.clear();
        }
        self.drafts.forget(id);
        self.refresh();
    }

    pub(super) fn stop_job(&mut self, id: u64) {
        self.assistance.stop_job(id);
        self.refresh();
    }

    pub(super) fn attach_all(&mut self, attached: Attached) {
        if attached.is_empty() {
            return skip_view();
        }
        let Attached {
            attachments,
            typed,
            refused,
        } = attached;
        self.attached.extend(attachments);
        if let Some(typed) = typed {
            self.draft.insert(&typed);
        }
        if let Some(reason) = refused {
            self.assistance.show_error(reason);
            self.refresh();
        }
    }

    pub(super) fn unattach(&mut self, at: usize) {
        if at < self.attached.len() {
            self.attached.remove(at);
        }
    }

    pub(crate) fn offer(&mut self) {
        match self.offers.typed(&self.draft) {
            DraftChange::Command(query) => {
                let offers = Builtin::matching(&query)
                    .map(Offer::builtin::<ShellChat>)
                    .chain(prompt_offers(self.assistance.hub(), &query))
                    .collect();
                self.offers.command(query, offers);
            }
            DraftChange::Mention(surveys) => {
                after(OFFER_DWELL, move |panel: &mut Panel| panel.look(surveys));
                skip_view();
            }
            DraftChange::Argument(surveys) => {
                after(OFFER_DWELL, move |panel: &mut Panel| {
                    panel.complete(surveys)
                });
                skip_view();
            }
            DraftChange::Cleared => {}
            DraftChange::Nothing => skip_view(),
        }
    }

    fn look(&mut self, surveys: u64) {
        let Some(query) = self.offers.pending_query(surveys, &self.draft) else {
            return skip_view();
        };
        let home = self.home.clone();
        let running = self.assistance.apps().running();
        let hub = self.assistance.hub().clone();
        spawn_blocking(
            move || {
                let mut offers = mention::offers(home.as_deref(), &running, &query);
                offers.extend(resource_offers(&hub, &query));
                offers
            },
            move |panel: &mut Panel, found: Vec<Offer>| panel.found(surveys, found),
        );
        skip_view();
    }

    fn complete(&mut self, surveys: u64) {
        let Some(argument) = self.offers.pending_argument(surveys, &self.draft) else {
            return skip_view();
        };
        spawn(
            argument_offers(self.assistance.hub().clone(), argument),
            move |panel: &mut Panel, found: Vec<Offer>| panel.found(surveys, found),
        );
        skip_view();
    }

    fn found(&mut self, surveys: u64, found: Vec<Offer>) {
        if !self.offers.found(surveys, &self.draft, found) {
            skip_view();
        }
    }

    pub(crate) fn step(&mut self, down: bool) {
        if !self.offers.step(down) {
            skip_view();
        }
    }

    pub(crate) fn point(&mut self, at: usize) {
        if !self.offers.point(at) {
            skip_view();
        }
    }

    pub(crate) fn dismiss(&mut self) {
        self.offers.dismiss();
    }

    pub(crate) fn take(&mut self, at: usize) {
        let Some(offer) = self.offers.take(at, &self.draft) else {
            return skip_view();
        };
        self.offer();
        if offer.runs {
            self.send();
        }
    }

    pub(super) fn agree(&mut self) {
        if !self.offers.agree(&self.draft) {
            return skip_view();
        }
        self.offer();
    }

    pub(crate) fn chose(&mut self, option: usize) {
        if let Some(asking) = self.asks.open_mut() {
            asking.choose(option);
        }
    }

    pub(crate) fn wrote(&mut self) {
        if let Some(asking) = self.asks.open_mut() {
            asking.wrote();
        }
    }

    pub(crate) fn back(&mut self) {
        if let Some(asking) = self.asks.open_mut() {
            asking.back();
        }
    }

    pub(crate) fn ahead(&mut self) {
        if let Some((id, picked)) = self.asks.ahead() {
            self.assistance.answer_question(id, picked);
            self.refresh();
        }
    }

    pub(super) fn pick_models(&mut self, open: bool) {
        self.pick(Picking::new(open));
    }

    pub(super) fn unfold_reasoning(&mut self, at: Option<f32>) {
        self.pick(self.picking.with_reasoning(at));
    }

    fn pick(&mut self, picking: Picking) {
        if picking == self.picking {
            return skip_view();
        }
        self.picking = picking;
    }

    pub(super) fn chose_model(&mut self, selection: Selection) {
        self.picking = Picking::new(false);
        self.assistance.choose(selection);
        self.refresh();
    }

    pub(super) fn chose_effort(&mut self, effort: Effort) {
        self.picking = Picking::new(false);
        let Some(chosen) = self.shown.selection.clone() else {
            return;
        };
        self.assistance
            .choose(Selection::new(chosen.account(), chosen.model(), effort));
        self.refresh();
    }

    pub(super) fn open_find(&mut self) {
        self.tab = Tab::Conversation;
        self.transcript.open_find();
    }

    pub(crate) fn find(&mut self, action: Action) {
        self.transcript.find(action);
    }

    pub(crate) fn close(&mut self) {
        self.seam.notify(Request::new(CAPABILITY, CLOSE));
        skip_view();
    }
}

impl std::fmt::Debug for Panel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Panel")
            .field("tab", &self.tab)
            .field("answering", &self.shown.is_answering())
            .finish_non_exhaustive()
    }
}
