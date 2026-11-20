use std::marker::PhantomData;

use rikka_format::format_count;
use rikka_llm::{ModelInfo, Trouble};
use rikka_llm_accounts::{open_verification, verification_label};
use rikka_thread::{Attachment, Job, QueuedAsk, is_near_context_limit};

use crate::attaching;
use crate::attachment;
use crate::chat::Chat;
use crate::jobs;
use crate::offering::Offering;
use crate::picker;
use crate::prelude::*;
use crate::question;
use crate::question_form::QuestionForm;
use crate::waiting;
use rikka_widgets::{column, row};

pub(crate) const DOCK: &str = "chat-dock";
const FOOT: &str = "chat-foot";
const BODY: &str = "chat-body";
const DRAFT: &str = "chat-draft";

const PANE: f32 = 20.0;
const READING: f32 = 760.0;
const LINE: f32 = 21.0;
const SHORTEST: f32 = 52.0;
const TALLEST: f32 = 190.0;
const RADIUS: f32 = Corner::Panel.radius();
const OPEN_RADIUS: f32 = Corner::Window.radius();
const GAUGE: f32 = 18.0;
const GLYPH: f32 = 20.0;

const TALL: f32 = 420.0;

const RISE: f32 = 8.0;

const ARRIVAL: Curve = Curve::Tween {
    easing: Easing::EmphasizedDecel,
    duration: DurationToken::Short4,
};

const WRITING: usize = 1;

const ASKING: usize = 0;

pub struct Composer<'a, C> {
    draft: &'a TextDocument,
    thread: &'a str,
    controls: Element,
    attached: &'a [Attachment],
    model: Option<&'a ModelInfo>,
    asking: Option<&'a QuestionForm>,
    instead: Option<(usize, Element)>,
    offering: Option<&'a Offering>,
    error: Option<&'a Trouble>,
    jobs: &'a [Job],
    watching: Option<u64>,
    waiting: &'a [QueuedAsk],
    screen: Option<(Element, bool)>,
    panels: Vec<Element>,
    chat: PhantomData<C>,
}

impl<'a, C: Chat> Composer<'a, C> {
    #[must_use]
    pub fn new(draft: &'a TextDocument, controls: impl View) -> Self {
        Composer {
            draft,
            thread: "",
            controls: controls.any(),
            attached: &[],
            model: None,
            asking: None,
            instead: None,
            offering: None,
            error: None,
            jobs: &[],
            watching: None,
            waiting: &[],
            screen: None,
            panels: Vec::new(),
            chat: PhantomData,
        }
    }

    #[must_use]
    pub fn thread(mut self, thread: &'a str) -> Self {
        self.thread = thread;
        self
    }

    #[must_use]
    pub fn attached(mut self, attached: &'a [Attachment]) -> Self {
        self.attached = attached;
        self
    }

    #[must_use]
    pub fn model(mut self, model: Option<&'a ModelInfo>) -> Self {
        self.model = model;
        self
    }

    #[must_use]
    pub fn asking(mut self, asking: Option<&'a QuestionForm>) -> Self {
        self.asking = asking;
        self
    }

    #[must_use]
    pub fn instead(mut self, index: usize, body: impl View) -> Self {
        self.instead = Some((index, body.any()));
        self
    }

    #[must_use]
    pub fn offering(mut self, offering: Option<&'a Offering>) -> Self {
        self.offering = offering;
        self
    }

    #[must_use]
    pub fn error(mut self, error: Option<&'a Trouble>) -> Self {
        self.error = error;
        self
    }

    #[must_use]
    pub fn jobs(mut self, jobs: &'a [Job], watching: Option<u64>) -> Self {
        self.jobs = jobs;
        self.watching = watching;
        self
    }

    #[must_use]
    pub fn waiting(mut self, waiting: &'a [QueuedAsk]) -> Self {
        self.waiting = waiting;
        self
    }

    #[must_use]
    pub fn screen(mut self, screen: impl View, tall: bool) -> Self {
        self.screen = Some((screen.any(), tall));
        self
    }

    #[must_use]
    pub fn panels(mut self, panels: Vec<Element>) -> Self {
        self.panels = panels;
        self
    }

    pub fn view(mut self) -> impl View + use<C> {
        let screen = self.screen.take();
        let panels = match self.asking.is_none() && self.instead.is_none() {
            true => std::mem::take(&mut self.panels),
            false => Vec::new(),
        };
        let offers = self.offering.map(picker::view::<C>);
        let tall = screen.as_ref().is_some_and(|(_, tall)| *tall);
        let is_open = offers.is_some();
        let foot = self.foot();

        let dock = column! {
            if let Some((screen, _)) = screen {
                screen;
                rule();
            }
            if let Some(offers) = offers {
                offers;
            }
            foot.keyed(FOOT);
        }
        .fill_width();
        let dock = match tall {
            true => dock.min_height(TALL),
            false => dock,
        };

        let surface = container(dock)
            .surface()
            .radius(radius(is_open))
            .border(1.0, palette().outline)
            .resizing_upward()
            .max_width(READING)
            .fill_width();

        container(
            column! {
                surface.build().keyed(DOCK);
                for panel in panels {
                    panel;
                }
            }
            .align_items(Align::Center)
            .fill_width(),
        )
        .padding(Inset::edges(0.0, PANE, PANE, PANE))
        .fill_width()
    }

    fn foot(self) -> Element {
        let Composer {
            draft,
            thread,
            controls,
            attached,
            model,
            asking,
            instead,
            offering,
            error,
            jobs,
            watching,
            waiting,
            ..
        } = self;
        let (showing, body) = match (asking, instead) {
            (Some(asking), _) => ((ASKING, asking.at()), question::view::<C>(asking).any()),
            (None, Some((shown, instead))) => ((shown, 0), instead),
            (None, None) => (
                (WRITING, 0),
                writing::<C>(Writing {
                    draft,
                    thread,
                    controls,
                    attached,
                    unseen: model.filter(|model| {
                        !model.vision()
                            && attached
                                .iter()
                                .any(|attached| attached.media().is_picture())
                    }),
                    picked: offering.map(Offering::selected),
                    watching: watching.is_some(),
                    recalled: waiting.last().map(QueuedAsk::id),
                })
                .any(),
            ),
        };

        container(
            column! {
                error.map(trouble);
                jobs::view::<C>(jobs, watching);
                waiting::view::<C>(waiting);
                body.fill_width()
                    .transition((fade() + offset(0.0, RISE)).enter(ARRIVAL))
                    .keyed((BODY, showing));
            }
            .gap(Space::SNUG)
            .fill_width(),
        )
        .padding(Space::NORMAL)
        .fill_width()
        .any()
    }
}

impl<C> std::fmt::Debug for Composer<'_, C> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Composer")
            .field("thread", &self.thread)
            .field("offering", &self.offering)
            .finish_non_exhaustive()
    }
}

fn radius(offering: bool) -> f32 {
    match offering {
        true => OPEN_RADIUS,
        false => RADIUS,
    }
}

fn trouble(trouble: &Trouble) -> Banner {
    let reported = banner(trouble.reason().to_owned()).tint(palette().danger);
    match trouble.verification_url() {
        Some(url) => {
            let url = url.to_owned();
            reported.on_action(verification_label(), move || open_verification(&url))
        }
        None => reported,
    }
}

struct Writing<'a> {
    draft: &'a TextDocument,
    thread: &'a str,
    controls: Element,
    attached: &'a [Attachment],
    unseen: Option<&'a ModelInfo>,
    picked: Option<usize>,
    watching: bool,
    recalled: Option<u64>,
}

fn writing<C: Chat>(writing: Writing<'_>) -> impl View + use<C> {
    let Writing {
        draft,
        thread,
        controls,
        attached,
        unseen,
        picked,
        watching,
        recalled,
    } = writing;
    let body = column! {
        if !attached.is_empty() {
            match watching {
                true => attachment::settled::<C>(attached),
                false => attachment::strip::<C>(attached),
            };
        }
        if let Some(model) = unseen.filter(|_| !watching) {
            attachment::unseen(model);
        }
        text_editor(draft)
            .placeholder(C::placeholder())
            .autofocus(!watching)
            .read_only(watching)
            .gutter(false)
            .family(Family::parse("sans-serif"))
            .font_size(14.0)
            .line_height(LINE)
            .fits_text(SHORTEST..=TALLEST)
            .on_change(C::offer())
            .on_submit(C::send())
            .on_paste(|mime_types: Vec<String>| match attaching::pasted_kind(&mime_types) {
                Some(content) => {
                    spawn_blocking(move || attaching::pasted::<C>(content), C::attached());
                    true
                }
                None => false,
            })
            .on_key(walking::<C>(Walking {
                picked,
                recalled,
                draft: draft.clone(),
            }))
            .build()
            .keyed((DRAFT, thread.to_owned()));
        controls;
    }
    .gap(Space::SNUG)
    .fill_width();
    drop_zone(body)
        .accepts(move |offer: &DragOffer| (!watching).then(|| attaching::accepts(offer)).flatten())
        .on_drop(|offer: DragOffer, _: DragAction| {
            spawn_blocking(move || attaching::dropped::<C>(&offer), C::attached());
        })
        .fill_width()
}

struct Walking {
    picked: Option<usize>,
    recalled: Option<u64>,
    draft: TextDocument,
}

fn walking<C: Chat>(walking: Walking) -> Handler<(Key, Modifiers), bool> {
    let Walking {
        picked,
        recalled,
        draft,
    } = walking;
    let (step, agree, take, dismiss) = (C::step(), C::agree(), C::take(), C::dismiss());
    let recall = C::recall();
    Handler::relaying(move |reach, (key, modifiers): (Key, Modifiers)| {
        if modifiers != Modifiers::NONE {
            return Some(false);
        }
        let Some(selected) = picked else {
            let empty = draft.text().is_empty();
            if let (Key::ArrowUp, true, Some(recall), Some(id)) = (key, empty, &recall, recalled) {
                recall.call(reach, (id,))?;
                return Some(true);
            }
            return Some(false);
        };
        match key {
            Key::ArrowUp => step.call(reach, (false,)),
            Key::ArrowDown => step.call(reach, (true,)),
            Key::Tab => agree.call(reach, ()),
            Key::Enter => take.call(reach, (selected,)),
            Key::Escape => dismiss.call(reach, ()),
            _ => return Some(false),
        }?;
        Some(true)
    })
}

pub fn outgoing_message<C: Chat>(
    answering: bool,
    draft: &TextDocument,
    attached: bool,
    pending_attachments: bool,
) -> Element {
    if answering && !C::interjects() {
        return stop::<C>().build();
    }
    let draft = draft.clone();
    live_observed(move || {
        let nothing = draft.text().trim().is_empty() && !attached;
        let sending = !answering || !nothing;
        row! {
            if answering {
                stop::<C>();
            }
            if sending {
                button(icon(MaterialSymbol::ARROW_UPWARD).extent(Length::px(GLYPH)))
                    .icon_only()
                    .enabled(!nothing && !pending_attachments)
                    .on_press(C::send());
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
    })
    .build()
}

fn stop<C: Chat>() -> Button {
    button(icon(MaterialSymbol::STOP_CIRCLE).extent(Length::px(GLYPH)))
        .icon_only()
        .secondary()
        .on_press(C::stop())
}

pub fn attach<C: Chat>(enabled: bool) -> Button {
    button(icon(MaterialSymbol::ADD).extent(Length::px(GLYPH)))
        .icon_only()
        .ghost()
        .enabled(enabled)
        .on_press(|| {
            let parent = window_identifier();
            spawn_blocking(move || attaching::chosen::<C>(parent), C::attached());
        })
}

pub fn context_fill(filled: u64, model: &ModelInfo) -> Option<Flex> {
    let context = model.context();
    if context == 0 {
        return None;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a share of a window, which no model counts in more than a float holds"
    )]
    let share = (filled as f32 / context as f32).clamp(0.0, 1.0);
    let crowded = is_near_context_limit(filled, model);
    let (ring, label) = match crowded {
        true => (palette().danger, palette().danger),
        false => (palette().accent, palette().text_muted),
    };

    Some(
        row! {
            text(format!("{} / {}", format_count(filled), format_count(context)))
                .caption()
                .color(label);
            gauge(share).extent(Length::px(GAUGE)).color(ring);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
}
