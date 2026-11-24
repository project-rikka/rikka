use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::stream::BoxStream;
use rikka_fuzzy::{Haystack, Needle, Score};
use rikka_seam::Request;
use rikka_ui::{IconSource, ImageSource, Label, MaterialSymbol};
use shell_kit::launch::Spawn;

pub(crate) const CERTAIN: Score = Score::new(1_000.0);

pub(crate) const LAST_RESORT: Score = Score::new(-1_000.0);

const ECHO_KEY: &str = "";

pub(super) type ChangeStream = BoxStream<'static, ()>;

pub(super) trait Provider: 'static {
    fn info(&self) -> ProviderInfo;

    fn reload(&mut self) {}

    fn search(&mut self, query: &Query) -> Vec<Hit>;

    fn listing(&mut self) -> Vec<Hit> {
        Vec::new()
    }

    fn recall(&mut self, _id: &str) -> Option<Hit> {
        None
    }

    fn activate(&mut self, hit: &Hit) -> Activation;

    fn within(&self) -> Option<Within> {
        None
    }

    fn leave(&mut self) {}

    fn changes(&self) -> Option<ChangeStream> {
        None
    }
}

pub(super) trait RemoteSearch: Send + 'static {
    fn info(&self) -> ProviderInfo;

    fn search(&mut self, query: &Query, token: &QueryToken) -> Vec<Hit>;

    fn taken(hit: &Hit) -> Activation
    where
        Self: Sized;
}

#[derive(Clone, Debug)]
pub(super) struct QueryToken {
    latest: Arc<AtomicU64>,
    answering: u64,
}

impl QueryToken {
    pub(crate) fn new(latest: Arc<AtomicU64>, answering: u64) -> QueryToken {
        QueryToken { latest, answering }
    }

    pub(crate) fn is_stale(&self) -> bool {
        self.latest.load(Ordering::Relaxed) != self.answering
    }
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub(crate) enum Layout {
    #[default]
    List,
    Gallery,
}

#[derive(Clone, Debug)]
pub(crate) enum Preview {
    Still(Option<ImageSource>),
    Motion(Arc<Path>),
}

#[derive(Clone, Debug)]
pub(crate) struct Within {
    opened_by: Arc<str>,
    layout: Layout,
}

impl Within {
    #[must_use]
    pub(crate) fn new(opened_by: Arc<str>, layout: Layout) -> Within {
        Within { opened_by, layout }
    }

    pub(crate) fn layout(&self) -> Layout {
        self.layout
    }

    pub(super) fn opened_by(&self) -> &str {
        &self.opened_by
    }
}

#[derive(Clone, Debug)]
pub(super) struct ProviderInfo {
    id: Arc<str>,
    label: Label,
    icon: MaterialSymbol,
    trigger: Option<&'static str>,
    remembered: bool,
}

impl ProviderInfo {
    #[must_use]
    pub(crate) fn new(id: impl Into<Arc<str>>, label: impl Into<Label>) -> ProviderInfo {
        ProviderInfo {
            id: id.into(),
            label: label.into(),
            icon: MaterialSymbol::SEARCH,
            trigger: None,
            remembered: true,
        }
    }

    #[must_use]
    pub(crate) fn icon(mut self, icon: MaterialSymbol) -> ProviderInfo {
        self.icon = icon;
        self
    }

    #[must_use]
    pub(crate) fn trigger(mut self, trigger: &'static str) -> ProviderInfo {
        self.trigger = Some(trigger);
        self
    }

    #[must_use]
    pub(super) fn without_history(mut self) -> ProviderInfo {
        self.remembered = false;
        self
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn label(&self) -> Label {
        self.label.retranslated()
    }

    pub(super) fn default_icon(&self) -> MaterialSymbol {
        self.icon
    }

    pub(crate) fn prefix(&self) -> Option<&'static str> {
        self.trigger
    }

    pub(crate) fn typed(&self, rest: &str) -> String {
        format!("{}{rest}", self.trigger.unwrap_or_default())
    }

    pub(super) fn is_remembered(&self) -> bool {
        self.remembered
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct Query {
    typed: String,
    text: String,
    needle: Needle,
    forced: bool,
}

impl Query {
    pub(crate) fn new(typed: &str) -> Query {
        let text = typed.trim().to_owned();
        Query {
            typed: typed.to_owned(),
            needle: Needle::new(&text),
            text,
            forced: false,
        }
    }

    #[must_use]
    pub(super) fn forced(mut self) -> Query {
        self.forced = true;
        self
    }

    pub(super) fn is_forced(&self) -> bool {
        self.forced
    }

    pub(crate) fn typed(&self) -> &str {
        &self.typed
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.needle.is_empty()
    }

    pub(super) fn score(&self, candidate: &str) -> Option<Score> {
        self.needle.score(candidate)
    }

    pub(crate) fn find(&self, haystack: &Haystack) -> Option<Score> {
        self.needle.find(haystack)
    }

    pub(super) fn best_of<'a>(
        &self,
        fields: impl IntoIterator<Item = (&'a str, f32)>,
    ) -> Option<Score> {
        fields
            .into_iter()
            .filter_map(|(candidate, weight)| Some(self.score(candidate)?.scaled(weight)))
            .max()
    }

    pub(super) fn best_found<'a>(
        &self,
        fields: impl IntoIterator<Item = &'a (Haystack, f32)>,
    ) -> Option<Score> {
        fields
            .into_iter()
            .filter_map(|(haystack, weight)| Some(self.find(haystack)?.scaled(*weight)))
            .max()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Hit {
    id: Box<str>,
    title: String,
    subtitle: Option<Label>,
    icon: Option<IconSource>,
    preview: Option<Preview>,
    completion: Option<Box<str>>,
    in_use: bool,
    echo: bool,
    arrival: Option<u64>,
    score: Score,
}

impl Hit {
    #[must_use]
    pub(crate) fn new(id: impl Into<Box<str>>, title: impl Into<String>) -> Hit {
        Hit {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            preview: None,
            completion: None,
            in_use: false,
            echo: false,
            arrival: None,
            score: Score::NEUTRAL,
        }
    }

    #[must_use]
    pub(super) fn echo(mut self) -> Hit {
        self.echo = true;
        self
    }

    #[must_use]
    pub(super) fn subtitle(mut self, subtitle: impl Into<Label>) -> Hit {
        self.subtitle = Some(subtitle.into());
        self
    }

    #[must_use]
    pub(crate) fn icon(mut self, icon: impl Into<IconSource>) -> Hit {
        self.icon = Some(icon.into());
        self
    }

    #[must_use]
    pub(crate) fn in_use(mut self) -> Hit {
        self.in_use = true;
        self
    }

    #[must_use]
    pub(crate) fn preview(mut self, preview: Preview) -> Hit {
        self.preview = Some(preview);
        self
    }

    #[must_use]
    pub(super) fn arrived(mut self, at: u64) -> Hit {
        self.arrival = Some(at);
        self
    }

    #[must_use]
    pub(super) fn score(mut self, score: Score) -> Hit {
        self.score = score;
        self
    }

    #[must_use]
    pub(super) fn completes(mut self, typed: impl Into<Box<str>>) -> Hit {
        self.completion = Some(typed.into());
        self
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn key(&self) -> &str {
        match self.echo {
            true => ECHO_KEY,
            false => &self.id,
        }
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(super) fn second_line(&self) -> Option<Label> {
        self.subtitle.as_ref().map(Label::retranslated)
    }

    pub(super) fn drawn_as(&self) -> Option<&IconSource> {
        self.icon.as_ref()
    }

    pub(super) fn completion(&self) -> Option<&str> {
        self.completion.as_deref()
    }

    pub(crate) fn picture(&self) -> Option<&Preview> {
        self.preview.as_ref()
    }

    pub(super) fn is_in_use(&self) -> bool {
        self.in_use
    }

    pub(super) fn arrival(&self) -> Option<u64> {
        self.arrival
    }

    pub(super) fn ranking(&self) -> Score {
        self.score
    }

    pub(crate) fn lift(&mut self, amount: f32) {
        self.score = self.score.offset(amount);
    }
}

#[derive(Debug)]
pub(crate) enum Activation {
    Ask(Box<Request>),
    Copy(String),
    Dismiss,
    Complete(Box<str>),
    Ignore,
}

impl Activation {
    #[must_use]
    pub(crate) fn ask(request: Request) -> Activation {
        Activation::Ask(Box::new(request))
    }

    #[must_use]
    pub(crate) fn spawn(program: Spawn) -> Activation {
        Activation::ask(program.request())
    }
}
