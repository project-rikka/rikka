use std::borrow::Borrow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;

use rikka_thread::{Answer, Attachment, Body, Call, Round, RoundSummary, Speaker, Thread, Turn};
use rikka_ui::search::{Action, Found};

use crate::amendment::Amendment;
use crate::attachment;
use crate::call::{self, CallContext};
use crate::call_summary::SummarizeCall;
use crate::card::{CARD_INSET, card, chevron};
use crate::chat::Chat;
use crate::composer::DOCK;
use crate::expanded::Expanded;
use crate::finding::{Block, Counting, Finding, Marks, Says, Searched};
use crate::mcp::kept_attachments;
use crate::prelude::*;
use crate::printed::PrintedDocuments;
use crate::spoken::{self, Amend};
use rikka_widgets::{column, row};

const READING: f32 = 760.0;
const GLYPH: f32 = 15.0;
const TURN: f32 = 20.0;
const ROUND: f32 = Corner::Panel.radius();
const STEP: f32 = 10.0;
const GUESS: f32 = 120.0;
const SENT_RISE: f32 = 24.0;

const ARRIVING: u64 = u64::MAX;

#[derive(State)]
pub struct Transcript {
    #[state]
    blocks: Blocks,
    expanded: Tracked<Rc<Expanded>>,
    finding: Tracked<Finding>,
    amendable: Tracked<bool>,
    amending: Tracked<Option<Amendment>>,
    #[untracked]
    documents: Rc<PrintedDocuments>,
    #[untracked]
    place: Place,
    #[untracked]
    summarizer: Rc<dyn SummarizeCall>,
}

impl Transcript {
    pub fn new(summarizer: impl SummarizeCall) -> Transcript {
        Transcript {
            blocks: Blocks::default(),
            expanded: Tracked::default(),
            finding: Tracked::default(),
            amendable: Tracked::new(false),
            amending: Tracked::new(None),
            documents: Rc::default(),
            place: Place::default(),
            summarizer: Rc::new(summarizer),
        }
    }

    pub fn refresh(&mut self, thread: Option<&Thread>, answering: Option<&Answer>) {
        self.blocks.refresh(thread, answering);
        self.recount();
    }

    pub fn stream(&self, answering: Option<&Answer>) -> bool {
        !self.finding.is_open() && self.blocks.stream(answering)
    }

    pub fn grow(&self, thread: Option<&Thread>, answering: Option<&Answer>) -> bool {
        !self.finding.is_open() && self.blocks.grow(thread, answering)
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn finding(&self) -> &Finding {
        &self.finding
    }

    pub fn forget(&mut self) {
        if !self.expanded.is_empty() {
            *self.expanded = Rc::default();
        }
        self.amending.set(None);
        self.documents.forget();
    }

    pub fn allow_amends(&mut self, allowed: bool) {
        self.amendable.set(allowed);
    }

    pub fn amend(&mut self, stamp: u64, text: &str) {
        if *self.amendable {
            self.amending.set(Some(Amendment::new(stamp, text)));
        }
    }

    pub fn amend_touched(&mut self, stamp: u64, paths: Vec<String>) {
        let Some(amending) = self.amending.as_ref().filter(|it| it.stamp() == stamp) else {
            return;
        };
        let mut measured = amending.clone();
        measured.touching(paths);
        self.amending.set(Some(measured));
    }

    #[must_use]
    pub fn amending(&self) -> Option<&Amendment> {
        self.amending.as_ref()
    }

    pub fn stop_amending(&mut self) {
        self.amending.set(None);
    }

    pub(crate) fn toggle_restore(&mut self) {
        let Some(amending) = self.amending.as_ref() else {
            return;
        };
        let mut toggled = amending.clone();
        toggled.toggle_restore();
        self.amending.set(Some(toggled));
    }

    pub fn open_find(&mut self) {
        self.finding.open(None);
        self.recount();
    }

    pub fn find(&mut self, action: Action) {
        self.finding.act(action);
        self.recount();
    }

    pub fn fold_output(&mut self, call: String) {
        Rc::make_mut(&mut *self.expanded).toggle(call);
    }

    pub fn toggle_summary(&mut self, turn: u64) {
        Rc::make_mut(&mut *self.expanded).toggle_summary(turn);
    }

    fn recount(&mut self) {
        if self.finding.is_idle() {
            return;
        }
        let Some(recount) = self.finding.recount(&self.blocks) else {
            return;
        };
        spawn_blocking(
            move || Arc::new(recount.count()),
            |transcript: &mut Transcript, counting: Arc<Counting>| {
                transcript.finding.counted(&counting);
            },
        );
    }
}

impl std::fmt::Debug for Transcript {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Transcript")
            .field("blocks", &self.blocks)
            .field("finding", &*self.finding)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Part {
    Whole,
    Said(usize),
    Call(usize, usize),
    Noticed,
    Text,
    Waiting,
    Folding,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Spot {
    turn: usize,
    part: Part,
    lead: f32,
}

#[derive(Clone, Copy, Debug, Hash)]
struct Named {
    thread: u64,
    turn: usize,
    part: Part,
}

impl Named {
    fn new(thread: u64, spot: Spot) -> Self {
        Self {
            thread,
            turn: spot.turn,
            part: spot.part,
        }
    }
}

#[derive(Debug, Default, State)]
pub(crate) struct Blocks {
    finished: Tracked<Arc<SettledBlocks>>,
    live: Tracked<Arc<Live>>,
    tailing: Tracked<bool>,
    #[untracked]
    streamed: MarkdownStream,
    #[untracked]
    revision: u64,
}

#[derive(Debug, Default, Clone)]
struct SettledBlocks {
    thread: u64,
    spots: Vec<Spot>,
    turns: Vec<Arc<Turn>>,
    sent: Option<usize>,
}

#[derive(Debug, Default)]
struct Live {
    spots: Vec<Spot>,
    rounds: Vec<Arc<Round>>,
}

impl Blocks {
    fn refresh(&mut self, thread: Option<&Thread>, answering: Option<&Answer>) -> bool {
        let mut moved = self.settle(thread);
        moved |= self.arrive(answering);
        if moved {
            self.revision += 1;
        }
        let tailing = !self.live.spots.is_empty();
        self.tailing.set(tailing);
        moved
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    fn stream(&self, answering: Option<&Answer>) -> bool {
        let Some((round, text)) = answering.map(Answer::open_round) else {
            return false;
        };
        self.streamed.grow(round, text)
    }

    fn grow(&self, thread: Option<&Thread>, answering: Option<&Answer>) -> bool {
        let Some(answer) = answering else {
            return false;
        };
        let turns = thread.map(Thread::turns).unwrap_or_default();
        self.finished.thread == contents(thread)
            && turns.len() == self.finished.turns.len()
            && follows(&self.finished.turns, turns)
            && self.live_spots(answer) == self.live.spots
            && only_said_more(answer.rounds(), &self.live.rounds)
            && self.stream(answering)
    }

    fn is_empty(&self) -> bool {
        self.finished.spots.is_empty() && !*self.tailing
    }

    pub(crate) fn says(&self) -> Vec<Block> {
        let settled_blocks =
            self.finished
                .spots
                .iter()
                .map(|spot| match self.finished.turns.get(spot.turn) {
                    Some(turn) => block(spot.part, turn),
                    None => arriving(spot.part, &self.live.rounds),
                });
        let live = self
            .live
            .spots
            .iter()
            .map(|spot| arriving(spot.part, &self.live.rounds));
        settled_blocks.chain(live).collect()
    }

    fn view<C: Chat>(&self, state: TranscriptState) -> TranscriptView<C> {
        TranscriptView {
            finished: Arc::clone(&self.finished),
            state,
            chat: std::marker::PhantomData,
        }
    }

    fn arriving<C: Chat>(&self, state: TranscriptState) -> Arriving<C> {
        Arriving {
            thread: self.finished.thread,
            after: self.finished.spots.len(),
            live: Arc::clone(&self.live),
            streamed: self.streamed.clone(),
            state,
            chat: std::marker::PhantomData,
        }
    }

    fn settle(&mut self, thread: Option<&Thread>) -> bool {
        let id = contents(thread);
        let turns = thread.map(Thread::turns).unwrap_or_default();
        if self.finished.thread != id || !follows(&self.finished.turns, turns) {
            *self.finished = Arc::new(SettledBlocks::new(id, turns));
            return true;
        }
        if turns.len() == self.finished.turns.len() {
            return false;
        }
        let blocks = Arc::make_mut(&mut *self.finished);
        blocks.sent = None;
        for (index, turn) in turns.iter().enumerate().skip(blocks.turns.len()) {
            if turn.speaker() == Speaker::User {
                blocks.sent = Some(blocks.spots.len());
            }
            let before = blocks.spots.last().map(|last| last.turn);
            turned(before, &mut blocks.spots, index, turn);
            blocks.turns.push(Arc::clone(turn));
        }
        true
    }

    fn arrive(&mut self, answering: Option<&Answer>) -> bool {
        match answering.map(Answer::open_round) {
            Some((round, text)) => self.streamed.follow(Some(round), text),
            None => self.streamed.follow(None, ""),
        }
        let Some(answer) = answering else {
            if self.live.spots.is_empty() && self.live.rounds.is_empty() {
                return false;
            }
            *self.live = Arc::default();
            return true;
        };
        let spots = self.live_spots(answer);
        if spots == self.live.spots && !said_more(answer.rounds(), &self.live.rounds) {
            return false;
        }
        *self.live = Arc::new(Live {
            spots,
            rounds: answer.rounds().to_vec(),
        });
        true
    }

    fn live_spots(&self, answer: &Answer) -> Vec<Spot> {
        let before = self.finished.spots.last().map(|last| last.turn);
        let turn = self.finished.turns.len();
        let mut spots = Vec::new();
        rounds_of(before, &mut spots, turn, live(answer));
        let running = answer
            .rounds()
            .iter()
            .flat_map(|round| round.calls())
            .any(Call::is_running);
        if answer.is_compacting() {
            spot(before, &mut spots, turn, Part::Folding, ROUND);
        } else if !running {
            spot(before, &mut spots, turn, Part::Waiting, ROUND);
        }
        spots
    }
}

impl SettledBlocks {
    fn new(thread: u64, turns: &[Arc<Turn>]) -> SettledBlocks {
        let mut spots = Vec::new();
        for (index, turn) in turns.iter().enumerate() {
            let before = spots.last().map(|last: &Spot| last.turn);
            turned(before, &mut spots, index, turn);
        }
        SettledBlocks {
            thread,
            spots,
            turns: turns.to_vec(),
            sent: None,
        }
    }
}

fn follows(previous: &[Arc<Turn>], turns: &[Arc<Turn>]) -> bool {
    previous.len() <= turns.len()
        && std::iter::zip(previous, turns).all(|(previous, turn)| Arc::ptr_eq(previous, turn))
}

fn said_more(rounds: &[Arc<Round>], previous: &[Arc<Round>]) -> bool {
    rounds.len() != previous.len()
        || std::iter::zip(rounds, previous)
            .any(|(round, previous)| round_counts(round) != round_counts(previous))
}

fn only_said_more(rounds: &[Arc<Round>], previous: &[Arc<Round>]) -> bool {
    let (Some((open, earlier)), Some((was, before))) = (rounds.split_last(), previous.split_last())
    else {
        return false;
    };
    earlier.len() == before.len()
        && std::iter::zip(earlier, before).all(|(round, previous)| Arc::ptr_eq(round, previous))
        && acted(open) == acted(was)
}

fn acted(round: &Round) -> (usize, usize, usize) {
    let (_, calls, output) = round_counts(round);
    let running = round
        .calls()
        .iter()
        .filter(|call| call.is_running())
        .count();
    (calls, output, running)
}

fn round_counts(round: &Round) -> (usize, usize, usize) {
    let output: usize = round
        .calls()
        .iter()
        .map(|call| call.output().map_or(0, str::len))
        .sum();
    (round.text().len(), round.calls().len(), output)
}

#[derive(Clone)]
struct TranscriptState {
    expanded: Rc<Expanded>,
    documents: Rc<PrintedDocuments>,
    summarizer: Rc<dyn SummarizeCall>,
    marks: Marks,
    amend: Amend,
}

impl TranscriptState {
    fn new(transcript: &Transcript) -> TranscriptState {
        TranscriptState {
            expanded: Rc::clone(&transcript.expanded),
            documents: Rc::clone(&transcript.documents),
            summarizer: Rc::clone(&transcript.summarizer),
            marks: transcript.finding.marks(),
            amend: Amend {
                allowed: *transcript.amendable,
                editing: (*transcript.amending).clone(),
            },
        }
    }

    fn call_context(&self) -> CallContext<'_> {
        CallContext {
            summarizer: &*self.summarizer,
            expanded: &self.expanded,
            documents: &self.documents,
        }
    }
}

pub fn transcript<C: Chat, V: View>(transcript: &Transcript, empty: impl FnOnce() -> V) -> Element {
    let blocks = &transcript.blocks;
    let settled_count = blocks.finished.spots.len();
    let tailing = *blocks.tailing;
    let count = settled_count + usize::from(tailing);
    if count == 0 {
        return empty().any();
    }

    let thread = blocks.finished.thread;
    let finding = &transcript.finding;
    let view = blocks.view::<C>(TranscriptState::new(transcript));

    let list = lazy_column(count, move |slot| match slot.index < settled_count {
        true => view.block(slot.index),
        false => component(|transcript: &Transcript| transcript, tail::<C>),
    })
    .padding(Space::LOOSE)
    .estimate(GUESS)
    .follow(true)
    .shows(thread)
    .reveals(finding.reveals().map(|block| block.min(settled_count)))
    .place(&transcript.place)
    .grow(1.0)
    .fill_width();
    jump_to_end(list, &transcript.place)
        .above(DOCK)
        .grow(1.0)
        .fill_width()
        .any()
}

fn tail<C: Chat>(transcript: &Transcript) -> impl View + use<C> {
    let arriving = transcript
        .blocks
        .arriving::<C>(TranscriptState::new(transcript));
    column_of(arriving.blocks().collect::<Vec<Element>>()).fill_width()
}

fn contents(thread: Option<&Thread>) -> u64 {
    let mut hasher = DefaultHasher::new();
    thread.map(Thread::id).unwrap_or_default().hash(&mut hasher);
    hasher.finish()
}

fn block(part: Part, turn: &Arc<Turn>) -> Block {
    let says = match part {
        Part::Whole if turn.speaker() == Speaker::User => {
            Says::Plain(Searched::Turn(Arc::clone(turn)))
        }
        Part::Text => Says::Marked(Searched::Turn(Arc::clone(turn))),
        Part::Said(round) => Says::Marked(Searched::Round(Arc::clone(turn), round)),
        Part::Whole | Part::Call(..) | Part::Noticed | Part::Waiting | Part::Folding => {
            Says::Nothing
        }
    };
    let mut hasher = DefaultHasher::new();
    turn.stamp().hash(&mut hasher);
    part.hash(&mut hasher);
    Block {
        stamp: hasher.finish(),
        says,
    }
}

fn arriving(part: Part, live: &[Arc<Round>]) -> Block {
    let says = match part {
        Part::Said(round) => match live.get(round) {
            Some(round) => Says::Marked(Searched::Arriving(Arc::clone(round))),
            None => Says::Nothing,
        },
        _ => Says::Nothing,
    };
    let content = match &says {
        Says::Nothing => 0,
        Says::Plain(content) | Says::Marked(content) => content.read(str::len),
    };
    let mut hasher = DefaultHasher::new();
    ARRIVING.hash(&mut hasher);
    part.hash(&mut hasher);
    content.hash(&mut hasher);
    Block {
        stamp: hasher.finish(),
        says,
    }
}

fn turned(before: Option<usize>, spots: &mut Vec<Spot>, index: usize, turn: &Turn) {
    match turn.speaker() {
        Speaker::Model => {
            rounds_of(
                before,
                spots,
                index,
                turn.round_summaries().iter().copied().enumerate(),
            );
            if turn.spoke() {
                spot(before, spots, index, Part::Text, ROUND);
            }
        }
        _ => spot(before, spots, index, Part::Whole, TURN),
    }
}

fn live(answer: &Answer) -> impl Iterator<Item = (usize, RoundSummary)> + '_ {
    answer
        .rounds()
        .iter()
        .enumerate()
        .filter(|(_, round)| !round.is_empty())
        .map(|(index, round)| (index, RoundSummary::new(round)))
}

fn rounds_of(
    before: Option<usize>,
    spots: &mut Vec<Spot>,
    turn: usize,
    rounds: impl Iterator<Item = (usize, RoundSummary)>,
) {
    for (index, round) in rounds {
        let count = spots.len();
        let step = |spots: &mut Vec<Spot>, part| {
            let lead = match spots.len() == count {
                true => ROUND,
                false => STEP,
            };
            spot(before, spots, turn, part, lead);
        };

        if round.has_text() {
            step(spots, Part::Said(index));
        }
        for at in 0..round.calls() {
            step(spots, Part::Call(index, at));
        }
        for _ in 0..round.notices() {
            step(spots, Part::Noticed);
        }
    }
}

fn spot(before: Option<usize>, spots: &mut Vec<Spot>, turn: usize, part: Part, lead: f32) {
    let lead = match spots.last().map(|last| last.turn).or(before) {
        None => 0.0,
        Some(last) if last != turn => TURN,
        Some(_) => lead,
    };
    spots.push(Spot { turn, part, lead });
}

struct TranscriptView<C> {
    finished: Arc<SettledBlocks>,
    state: TranscriptState,
    chat: std::marker::PhantomData<C>,
}

impl<C: Chat> TranscriptView<C> {
    fn block(&self, index: usize) -> Element {
        let Some(spot) = self.finished.spots.get(index).copied() else {
            return spacer().any();
        };
        let Some(turn) = self.finished.turns.get(spot.turn) else {
            return spacer().any();
        };
        let (marked, found) = self.state.marks.at(index);
        let named = Named::new(self.finished.thread, spot);
        let turn = Arc::clone(turn);
        let state = self.state.clone();
        let sent = self.finished.sent == Some(index);
        let stamp = stamp(spot, &turn, &state, marked, sent);
        lazy(stamp, move || {
            let body = turn.body();
            let content = settled_turn::<C>(named, &turn, &body, &state, &found);
            let content = match sent {
                true => content.transition(
                    (fade() + offset(0.0, SENT_RISE)).enter(Curve::Spring(Spring::DEFAULT_SPATIAL)),
                ),
                false => content,
            };
            read(spot.lead, content)
        })
        .fill_width()
        .any()
    }
}

struct Arriving<C> {
    thread: u64,
    after: usize,
    live: Arc<Live>,
    streamed: MarkdownStream,
    state: TranscriptState,
    chat: std::marker::PhantomData<C>,
}

impl<C> Clone for Arriving<C> {
    fn clone(&self) -> Self {
        Arriving {
            thread: self.thread,
            after: self.after,
            live: Arc::clone(&self.live),
            streamed: self.streamed.clone(),
            state: self.state.clone(),
            chat: std::marker::PhantomData,
        }
    }
}

impl<C: Chat> Arriving<C> {
    fn blocks(&self) -> impl Iterator<Item = Element> + '_ {
        (0..self.live.spots.len()).map(|at| self.block(at))
    }

    fn block(&self, at: usize) -> Element {
        let Some(spot) = self.live.spots.get(at).copied() else {
            return spacer().any();
        };
        let (marked, found) = self.state.marks.at(self.after + at);
        let named = Named::new(self.thread, spot);
        let arriving = self.clone();
        let block = lazy(self.stamp(spot, marked), move || {
            read(spot.lead, arriving.content(named, &found))
        })
        .fill_width()
        .build();
        match spot.part {
            Part::Call(..) => block.keyed(spot.part),
            _ => block,
        }
    }

    fn stamp(&self, spot: Spot, marked: u64) -> u64 {
        let mut hasher = DefaultHasher::new();
        ARRIVING.hash(&mut hasher);
        self.thread.hash(&mut hasher);
        spot.lead.to_bits().hash(&mut hasher);
        marked.hash(&mut hasher);
        spot.part.hash(&mut hasher);
        let rounds = &self.live.rounds;
        match spot.part {
            Part::Said(round) if self.streamed.carries(round) => true.hash(&mut hasher),
            Part::Said(round) => round_text(rounds, round).hash(&mut hasher),
            Part::Call(round, at) => {
                if let Some(call) = called(rounds, round, at) {
                    call.id().hash(&mut hasher);
                    call.is_running().hash(&mut hasher);
                    call.is_failed().hash(&mut hasher);
                    call.output().map(str::len).hash(&mut hasher);
                    call.detail().is_some().hash(&mut hasher);
                    self.state.expanded.holds(call.id()).hash(&mut hasher);
                }
            }
            Part::Whole | Part::Text | Part::Noticed | Part::Waiting | Part::Folding => {}
        }
        hasher.finish()
    }

    fn content(&self, named: Named, found: &Found) -> Element {
        let rounds = &self.live.rounds;
        match named.part {
            Part::Said(round) if self.streamed.carries(round) => {
                prose(streamed_markdown(&self.streamed), named, found)
            }
            Part::Said(round) => prose(markdown(round_text(rounds, round)), named, found),
            Part::Noticed => noticed::<C>().any(),
            Part::Call(round, at) => match called(rounds, round, at) {
                Some(call) => call::arriving::<C>(call, &self.state.call_context()),
                None => spacer().any(),
            },
            Part::Waiting => waiting(C::answering()).any(),
            Part::Folding => waiting(C::folding()).any(),
            Part::Whole | Part::Text => spacer().any(),
        }
    }
}

fn read(lead: f32, content: Element) -> Element {
    container(
        column! {
            content;
        }
        .max_width(READING)
        .fill_width(),
    )
    .align_x(Align::Center)
    .padding(Inset::edges(lead, 0.0, 0.0, 0.0))
    .fill_width()
    .any()
}

fn stamp(spot: Spot, turn: &Turn, state: &TranscriptState, marked: u64, sent: bool) -> u64 {
    let expanded = &state.expanded;
    let mut hasher = DefaultHasher::new();
    spot.lead.to_bits().hash(&mut hasher);
    marked.hash(&mut hasher);
    turn.stamp().hash(&mut hasher);
    spot.part.hash(&mut hasher);
    sent.hash(&mut hasher);
    if let Part::Call(round, at) = spot.part {
        let body = turn.body();
        let open = called(body.rounds(), round, at).is_some_and(|call| expanded.holds(call.id()));
        open.hash(&mut hasher);
    }
    match turn.speaker() {
        Speaker::Summary => expanded.holds_summary(turn.stamp()).hash(&mut hasher),
        Speaker::User => state.amend.mark(turn.stamp(), &mut hasher),
        Speaker::Model | Speaker::Notice => {}
    }
    hasher.finish()
}

fn round_text<R: Borrow<Round>>(rounds: &[R], round: usize) -> &str {
    rounds
        .get(round)
        .map(|round| round.borrow().text())
        .unwrap_or_default()
}

fn called<R: Borrow<Round>>(rounds: &[R], round: usize, at: usize) -> Option<&Call> {
    rounds.get(round)?.borrow().calls().get(at)
}

fn settled_turn<C: Chat>(
    named: Named,
    turn: &Turn,
    body: &Body,
    state: &TranscriptState,
    found: &Found,
) -> Element {
    match named.part {
        Part::Whole => whole::<C>(turn, body, state, named, found),
        Part::Text => prose(markdown(body.text()), named, found),
        Part::Said(round) => prose(markdown(round_text(body.rounds(), round)), named, found),
        Part::Noticed => noticed::<C>().any(),
        Part::Call(round, at) => match called(body.rounds(), round, at) {
            Some(call) => call::view::<C>(call, &state.call_context()),
            None => spacer().any(),
        },
        Part::Waiting => waiting(C::answering()).any(),
        Part::Folding => waiting(C::folding()).any(),
    }
}

fn prose(markdown: Markdown, named: Named, found: &Found) -> Element {
    markdown
        .selectable(named)
        .found(found.clone())
        .fill_width()
        .any()
}

fn whole<C: Chat>(
    turn: &Turn,
    body: &Body,
    state: &TranscriptState,
    named: Named,
    found: &Found,
) -> Element {
    match turn.speaker() {
        Speaker::User => {
            let given = kept_attachments(body.text(), body.sent_text(), body.attachments());
            match (state.amend.editing(turn.stamp()), C::amend()) {
                (Some(amending), Some(amends)) => {
                    spoken::editor::<C>(amending, amends, &given, state.amend.allowed)
                }
                _ => spoken::person::<C>(
                    bubble::<C>(body.text(), &given, named, found),
                    body.text(),
                    turn.stamp(),
                    state.amend.allowed,
                ),
            }
        }
        Speaker::Summary => summarised::<C>(
            body,
            turn.stamp(),
            state.expanded.holds_summary(turn.stamp()),
            named,
        ),
        Speaker::Model | Speaker::Notice => noticed::<C>().any(),
    }
}

fn bubble<C: Chat>(said: &str, given: &[Attachment], named: Named, found: &Found) -> Element {
    container(
        column! {
            if !given.is_empty() {
                attachment::settled::<C>(given);
            }
            if !said.is_empty() {
                rich_text(said)
                    .selectable(named)
                    .found(found.clone())
                    .label()
                    .color(palette().on_selection);
            }
        }
        .gap(Space::SNUG),
    )
    .background(palette().selection)
    .radius(Corner::Panel)
    .padding(Inset::xy(14.0, 10.0))
    .any()
}

fn summarised<C: Chat>(body: &Body, stamp: u64, open: bool, named: Named) -> Element {
    let heading = container(aside(
        MaterialSymbol::COMPRESS,
        C::folded(),
        Some(chevron(open)),
    ))
    .fill_width()
    .on_press(move |transcript: &mut Transcript| transcript.toggle_summary(stamp))
    .focusless();

    column! {
        heading;
        if open {
            card(
                column! {
                    markdown(body.text()).selectable(named).fill_width();
                },
                Inset::xy(CARD_INSET, 10.0),
                palette().outline,
            );
        }
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn noticed<C: Chat>() -> impl View {
    aside(MaterialSymbol::NOTIFICATIONS_ACTIVE, C::noticed(), None)
}

fn aside(symbol: MaterialSymbol, label: Label, after: Option<MaterialSymbol>) -> Flex {
    row! {
        rule().width(Length::HUG).grow(1.0);
        icon(symbol)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted);
        text(label).caption().muted();
        if let Some(after) = after {
            icon(after)
                .extent(Length::px(GLYPH))
                .color(palette().text_muted);
        }
        rule().width(Length::HUG).grow(1.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn waiting(status: Label) -> impl View {
    row! {
        spinner().extent(Length::px(GLYPH));
        text(status).label().muted();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}
