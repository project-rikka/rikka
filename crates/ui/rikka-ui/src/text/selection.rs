use std::cell::{Cell, RefCell};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use rikka_text::{Affinity, TextLayout, WordRule, word_range_at};
use rikka_text_document::{Endpoints, Granularity, SelectionOrigin};
use rikka_types::{Point, Rect};
use rustc_hash::{FxHashMap, FxHasher};

use crate::runtime::Watchers;

const SWEPT: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Region(u64);

impl Region {
    pub fn new(key: impl Hash) -> Self {
        Self(digest(|hasher| key.hash(hasher)))
    }
}
#[derive(Debug, Default)]
struct Whole {
    text: RefCell<String>,
    stamp: Cell<u64>,
}

#[derive(Debug)]
pub struct SharedWhole {
    region: Region,
    text: RefCell<Arc<str>>,
    stamp: Cell<u64>,
}

impl SharedWhole {
    #[must_use]
    pub fn for_region(region: Region) -> Rc<SharedWhole> {
        WHOLES.with(|wholes| wholes.borrow_mut().get_or_insert(region))
    }

    pub fn set(&self, text: &Arc<str>) {
        if Arc::ptr_eq(&self.text.borrow(), text) {
            return;
        }
        self.stamp.set(digest(|hasher| text.hash(hasher)));
        *self.text.borrow_mut() = Arc::clone(text);
    }
}

#[derive(Default)]
struct Wholes {
    wholes: FxHashMap<Region, Weak<SharedWhole>>,
    swept_at: usize,
}

impl Wholes {
    fn get_or_insert(&mut self, region: Region) -> Rc<SharedWhole> {
        if let Some(whole) = self.wholes.get(&region).and_then(Weak::upgrade) {
            return whole;
        }
        if self.wholes.len() >= self.swept_at {
            self.wholes.retain(|_, whole| whole.strong_count() > 0);
            self.swept_at = (self.wholes.len() * 2).max(SWEPT);
        }
        let text: Arc<str> = Arc::from("");
        let whole = Rc::new(SharedWhole {
            region,
            stamp: Cell::new(digest(|hasher| text.hash(hasher))),
            text: RefCell::new(text),
        });
        self.wholes.insert(region, Rc::downgrade(&whole));
        whole
    }
}

#[derive(Clone, Debug)]
enum Draft {
    Growing(Rc<Whole>),
    Shared(Rc<SharedWhole>),
}

impl Default for Draft {
    fn default() -> Self {
        Draft::Growing(Rc::default())
    }
}

impl Draft {
    fn grow(whole: &Whole, lead: &str, text: &str) -> usize {
        let mut buffer = whole.text.borrow_mut();
        if !buffer.is_empty() {
            buffer.push_str(lead);
        }
        let start = buffer.len();
        buffer.push_str(text);
        let was = whole.stamp.get();
        whole.stamp.set(digest(|hasher| {
            was.hash(hasher);
            lead.hash(hasher);
            text.hash(hasher);
        }));
        start
    }

    fn text(&self) -> Arc<str> {
        match self {
            Draft::Growing(whole) => Arc::from(whole.text.borrow().as_str()),
            Draft::Shared(whole) => Arc::clone(&whole.text.borrow()),
        }
    }

    fn read<T>(&self, read: impl FnOnce(&str) -> T) -> T {
        match self {
            Draft::Growing(whole) => read(&whole.text.borrow()),
            Draft::Shared(whole) => read(&whole.text.borrow()),
        }
    }

    fn stamp(&self) -> u64 {
        match self {
            Draft::Growing(whole) => whole.stamp.get(),
            Draft::Shared(whole) => whole.stamp.get(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Gather {
    region: Region,
    draft: Draft,
    lead: Cell<&'static str>,
    from: usize,
    at: Cell<usize>,
}

impl Gather {
    pub fn new(region: Region) -> Self {
        Self {
            region,
            draft: Draft::default(),
            lead: Cell::new(""),
            from: 0,
            at: Cell::new(0),
        }
    }

    pub fn shared(whole: &Rc<SharedWhole>, from: usize) -> Self {
        Self {
            region: whole.region,
            draft: Draft::Shared(Rc::clone(whole)),
            lead: Cell::new(""),
            from,
            at: Cell::new(from),
        }
    }

    pub fn opens(&self, lead: &'static str) {
        self.lead.set(lead);
    }

    pub fn add(&self, text: &str) -> Part {
        let lead = self.lead.replace("");
        let start = match &self.draft {
            Draft::Growing(whole) => Draft::grow(whole, lead, text),
            Draft::Shared(_) => match self.at.get() {
                at if at == self.from => at,
                at => at + lead.len(),
            },
        };
        self.at.set(start + text.len());
        Part {
            region: self.region,
            start,
            len: text.len(),
            draft: self.draft.clone(),
        }
    }

    pub fn skip(&self) {
        self.add("");
    }

    pub fn text(&self) -> Arc<str> {
        self.draft.text()
    }
}
#[derive(Clone, Debug)]
pub struct Part {
    region: Region,
    draft: Draft,
    start: usize,
    len: usize,
}

impl Part {
    pub fn whole(region: Region, text: &str) -> Self {
        Gather::new(region).add(text)
    }

    pub fn subrange(&self, range: Range<usize>) -> Self {
        let start = self.start + range.start.min(self.len);
        Self {
            region: self.region,
            draft: self.draft.clone(),
            start,
            len: range.len().min(self.ends().saturating_sub(start)),
        }
    }

    pub fn absolute(&self, offset: usize) -> usize {
        self.start + offset
    }

    pub fn read_text<T>(&self, read: impl FnOnce(&str) -> T) -> T {
        self.draft.read(read)
    }

    pub fn stamp(&self) -> u64 {
        self.draft.stamp()
    }

    pub fn window(&self) -> Range<usize> {
        self.start..self.ends()
    }

    fn ends(&self) -> usize {
        self.start + self.len
    }
}

struct Session {
    region: Region,
    whole: Arc<str>,
    stamp: u64,
    owner: u64,
    origin: SelectionOrigin,
    anchor: usize,
    focus: usize,
}

impl Session {
    fn new(part: &Part, at: usize) -> Self {
        let whole = part.draft.text();
        let at = whole.floor_char_boundary(at);
        Self {
            region: part.region,
            stamp: part.draft.stamp(),
            whole,
            owner: 0,
            origin: SelectionOrigin::new(Granularity::Character, at..at),
            anchor: at,
            focus: at,
        }
    }

    fn range(&self) -> Range<usize> {
        match self.anchor <= self.focus {
            true => self.anchor..self.focus,
            false => self.focus..self.anchor,
        }
    }

    fn unit_at(&self, at: usize, affinity: Affinity, granularity: Granularity) -> Range<usize> {
        let at = self.whole.floor_char_boundary(at);
        match granularity {
            Granularity::Character => at..at,
            Granularity::Word => word_range_at(&*self.whole, at, affinity, WordRule::Prose),
            Granularity::Line => line_at(&self.whole, at),
        }
    }

    fn start(&mut self, at: usize, affinity: Affinity, granularity: Granularity) {
        let unit = self.unit_at(at, affinity, granularity);
        self.anchor = unit.start;
        self.focus = unit.end;
        self.origin = SelectionOrigin::new(granularity, unit);
    }

    fn extend(&mut self, at: usize, affinity: Affinity) {
        let unit = self.unit_at(at, affinity, self.origin.granularity());
        let Endpoints { anchor, focus } = self.origin.extend(unit);
        self.anchor = anchor;
        self.focus = focus;
    }
}

struct PaintedText {
    placement: u64,
    part: Part,
    bounds: Rect,
    laid: Arc<TextLayout>,
}

impl PaintedText {
    fn cursor_at(&self, whole: &str, at: Point) -> (usize, Affinity) {
        let (reached, affinity) = if at.y < self.bounds.y() {
            (self.part.start, Affinity::After)
        } else if at.y >= self.bounds.max_y() {
            (self.part.ends(), Affinity::Before)
        } else {
            let cursor = self
                .laid
                .hit(at.x - self.bounds.x(), at.y - self.bounds.y());
            (
                self.part.absolute(self.laid.offset_of(cursor)),
                cursor.affinity,
            )
        };
        (whole.floor_char_boundary(reached), affinity)
    }

    fn away_from(&self, at: Point) -> (f32, f32) {
        let down = (self.bounds.y() - at.y)
            .max(at.y - self.bounds.max_y())
            .max(0.0);
        let across = (self.bounds.x() - at.x)
            .max(at.x - self.bounds.max_x())
            .max(0.0);
        (down, across)
    }
}

#[derive(Default)]
struct Selection {
    session: Option<Session>,
    drawn: Vec<PaintedText>,
    painters: FxHashMap<Region, Watchers>,
    presses: u64,
    placements: u64,
}

impl Selection {
    fn wholes(&self, part: &Part) -> Option<&Session> {
        self.session
            .as_ref()
            .filter(|wholes| wholes.region == part.region)
    }

    fn keeps(&self, part: &Part) -> bool {
        self.wholes(part)
            .is_some_and(|wholes| wholes.stamp == part.draft.stamp())
    }

    fn settle(&mut self, part: &Part) {
        let stamp = part.draft.stamp();
        let Some(wholes) = self
            .session
            .as_mut()
            .filter(|wholes| wholes.region == part.region && wholes.stamp != stamp)
        else {
            return;
        };
        let whole = part.draft.text();
        let reach = wholes.anchor.max(wholes.focus);
        if whole.get(..reach) != wholes.whole.get(..reach) {
            self.session = None;
            return;
        }
        wholes.whole = whole;
        wholes.stamp = stamp;
    }

    fn note(&mut self, drawn: PaintedText) -> Watchers {
        let region = drawn.part.region;
        match self
            .drawn
            .iter_mut()
            .find(|entry| entry.placement == drawn.placement)
        {
            Some(entry) => *entry = drawn,
            None => self.drawn.push(drawn),
        }
        self.painters.entry(region).or_default().clone()
    }

    fn shift(&mut self, placement: u64, by: Point) {
        if let Some(drawn) = self
            .drawn
            .iter_mut()
            .find(|drawn| drawn.placement == placement)
        {
            drawn.bounds = drawn.bounds.translate(by);
        }
    }

    fn unplace(&mut self, placement: u64) {
        let Some(at) = self
            .drawn
            .iter()
            .position(|drawn| drawn.placement == placement)
        else {
            return;
        };
        let region = self.drawn.swap_remove(at).part.region;
        if !self.drawn.iter().any(|drawn| drawn.part.region == region) {
            self.painters.remove(&region);
        }
    }

    fn marked(&self) -> Option<(Region, usize, usize)> {
        self.session
            .as_ref()
            .map(|session| (session.region, session.anchor, session.focus))
    }

    fn session(&mut self, part: &Part, at: usize) -> (&mut Session, u64) {
        self.presses += 1;
        self.settle(part);
        let existing_session = match self.keeps(part) {
            true => self.session.take(),
            false => None,
        };
        let owner = self.presses;
        let session = self
            .session
            .insert(existing_session.unwrap_or_else(|| Session::new(part, at)));
        session.owner = owner;
        (session, owner)
    }
    fn cursor_at(&self, part: &Part, at: Point) -> Option<(usize, Affinity)> {
        let whole = &self.wholes(part)?.whole;
        let mut nearest: Option<((f32, f32), &PaintedText)> = None;
        for drawn in self
            .drawn
            .iter()
            .filter(|drawn| drawn.part.region == part.region)
        {
            if drawn.bounds.contains(at) {
                return Some(drawn.cursor_at(whole, at));
            }
            let away = drawn.away_from(at);
            if nearest.is_none_or(|(closest, _)| away < closest) {
                nearest = Some((away, drawn));
            }
        }
        Some(nearest?.1.cursor_at(whole, at))
    }
}

thread_local! {
    static SELECTION: RefCell<Selection> = RefCell::new(Selection::default());
    static WHOLES: RefCell<Wholes> = RefCell::new(Wholes::default());
}

fn with<T>(read: impl FnOnce(&mut Selection) -> T) -> T {
    SELECTION.with(|selection| read(&mut selection.borrow_mut()))
}

fn altering<T>(change: impl FnOnce(&mut Selection) -> T) -> T {
    let (result, stirred) = with(|selection| {
        let before = selection.marked();
        let result = change(selection);
        let after = selection.marked();
        let stirred: Vec<Watchers> = match before == after {
            true => Vec::new(),
            false => [before, after]
                .into_iter()
                .flatten()
                .filter_map(|(region, _, _)| selection.painters.get(&region).cloned())
                .collect(),
        };
        (result, stirred)
    });
    for painters in stirred {
        painters.repaint();
    }
    result
}

fn digest(write: impl FnOnce(&mut FxHasher)) -> u64 {
    let mut hasher = FxHasher::default();
    write(&mut hasher);
    hasher.finish()
}

#[derive(Debug)]
pub struct Placement(u64);

impl Default for Placement {
    fn default() -> Self {
        Placement(with(|selection| {
            selection.placements += 1;
            selection.placements
        }))
    }
}

impl Placement {
    pub fn note(&self, part: &Part, bounds: Rect, laid: &Arc<TextLayout>) {
        let painters = altering(|selection| {
            selection.settle(part);
            selection.note(PaintedText {
                placement: self.0,
                part: part.clone(),
                bounds,
                laid: Arc::clone(laid),
            })
        });
        painters.watch_current();
    }

    pub fn shift(&self, by: Point) {
        with(|selection| selection.shift(self.0, by));
    }
}

impl Drop for Placement {
    fn drop(&mut self) {
        let _ = SELECTION.try_with(|selection| {
            if let Ok(mut selection) = selection.try_borrow_mut() {
                selection.unplace(self.0);
            }
        });
    }
}

pub fn press(part: &Part, at: usize, affinity: Affinity, granularity: Granularity) -> u64 {
    altering(|selection| {
        let (session, owner) = selection.session(part, at);
        session.start(at, affinity, granularity);
        owner
    })
}

pub fn extend(part: &Part, at: usize, affinity: Affinity) -> u64 {
    altering(|selection| {
        let (session, owner) = selection.session(part, at);
        session.extend(at, affinity);
        owner
    })
}

pub fn drag(part: &Part, at: Point) -> bool {
    altering(|selection| {
        let Some((offset, affinity)) = selection.cursor_at(part, at) else {
            return false;
        };
        selection.session.as_mut().is_some_and(|session| {
            let before = (session.anchor, session.focus);
            session.extend(offset, affinity);
            before != (session.anchor, session.focus)
        })
    })
}

pub fn select_all(part: &Part) -> Option<u64> {
    altering(|selection| {
        let (session, owner) = selection.session(part, 0);
        let ends = session.whole.len();
        if ends == 0 {
            return None;
        }
        session.origin = SelectionOrigin::new(Granularity::Character, 0..0);
        session.anchor = 0;
        session.focus = ends;
        Some(owner)
    })
}

pub fn selected(part: &Part) -> Option<String> {
    with(|selection| {
        let session = selection.wholes(part)?;
        session
            .whole
            .get(session.range())
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    })
}

pub fn range_in(part: &Part) -> Option<Range<usize>> {
    with(|selection| {
        let range = selection.wholes(part)?.range();
        let from = range.start.max(part.start);
        let to = range.end.min(part.ends());
        (from < to).then(|| from - part.start..to - part.start)
    })
}

pub fn adopt(part: &Part) -> Option<u64> {
    with(|selection| {
        selection.settle(part);
        if !selection.keeps(part) {
            return None;
        }
        selection.presses += 1;
        let owner = selection.presses;
        selection.session.as_mut()?.owner = owner;
        Some(owner)
    })
}

pub fn holds(part: &Part) -> bool {
    with(|selection| {
        selection
            .wholes(part)
            .is_some_and(|wholes| !wholes.range().is_empty())
    })
}

pub fn forget(part: &Part) -> bool {
    altering(|selection| {
        let wholes = selection.wholes(part).is_some();
        if wholes {
            selection.session = None;
        }
        wholes
    })
}

pub fn forget_owned(part: &Part, owner: u64) -> bool {
    altering(|selection| {
        let owned = selection
            .wholes(part)
            .is_some_and(|wholes| wholes.owner == owner);
        if owned {
            selection.session = None;
        }
        owned
    })
}

fn line_at(text: &str, at: usize) -> Range<usize> {
    let start = text[..at].rfind('\n').map_or(0, |break_at| break_at + 1);
    let end = text[at..]
        .find('\n')
        .map_or(text.len(), |break_at| at + break_at);
    start..end
}
