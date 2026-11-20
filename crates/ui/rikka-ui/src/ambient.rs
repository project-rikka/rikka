use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::rc::Rc;
use std::time::Instant;

use rikka_theme::{Palette, Theme};

use crate::view::Env;

pub(crate) type ThemeHandle = Rc<Theme>;

thread_local! {
    static THEME: RefCell<Option<ThemeHandle>> = const { RefCell::new(None) };
    static ENV: Cell<Option<Env>> = const { Cell::new(None) };
    static KEEPING: Cell<bool> = const { Cell::new(false) };
    static PREFETCHING: Cell<Option<Instant>> = const { Cell::new(None) };
}

pub fn theme() -> ThemeRef {
    ThemeRef(current_theme())
}

pub fn palette() -> Palette {
    THEME.with(|theme| standing(&theme.borrow()).palette())
}

pub fn env() -> Env {
    ENV.with(Cell::get).expect(WHILE_SHOWN)
}

pub fn is_exiting() -> bool {
    env().leaving
}

pub fn can_retain() -> bool {
    KEEPING.with(Cell::get)
}

pub(crate) fn keeping<R>(allowed: bool, build: impl FnOnce() -> R) -> R {
    let was = KEEPING.with(|keeping| keeping.replace(allowed));
    let built = build();
    KEEPING.with(|keeping| keeping.set(was));
    built
}

pub fn prefetch_deadline() -> Option<Instant> {
    PREFETCHING.with(Cell::get)
}

pub(crate) fn prefetching_until<R>(until: Instant, work: impl FnOnce() -> R) -> R {
    let was = PREFETCHING.with(|prefetching| prefetching.replace(Some(until)));
    let done = work();
    PREFETCHING.with(|prefetching| prefetching.set(was));
    done
}

#[derive(Clone)]
pub struct ThemeRef(ThemeHandle);

impl Deref for ThemeRef {
    type Target = Theme;

    fn deref(&self) -> &Theme {
        &self.0
    }
}

impl std::fmt::Debug for ThemeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[inline]
pub(crate) fn with_env<R>(theme: &ThemeHandle, env: Env, body: impl FnOnce() -> R) -> R {
    let is_current = THEME.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|current| Rc::ptr_eq(current, theme))
    });
    let settled = is_current && ENV.get() == Some(env);
    let _theme =
        (!is_current).then(|| Restore(THEME.with(|slot| slot.replace(Some(theme.clone())))));
    let _env = (!settled).then(|| Surroundings(ENV.replace(Some(env))));
    body()
}

struct Restore(Option<ThemeHandle>);

impl Drop for Restore {
    fn drop(&mut self) {
        THEME.with(|theme| *theme.borrow_mut() = self.0.take());
    }
}

struct Surroundings(Option<Env>);

impl Drop for Surroundings {
    fn drop(&mut self) {
        ENV.set(self.0);
    }
}

pub fn with_theme<R>(theme: &Theme, body: impl FnOnce() -> R) -> R {
    let theme = ThemeHandle::new(theme.clone());
    let env = Env::from_theme(&theme);
    with_env(&theme, env, body)
}

fn current_theme() -> ThemeHandle {
    THEME.with(|theme| standing(&theme.borrow()).clone())
}

fn standing(theme: &Option<ThemeHandle>) -> &ThemeHandle {
    theme.as_ref().expect(WHILE_SHOWN)
}

const WHILE_SHOWN: &str = "the surrounding theme is only here while a surface is being viewed, built, measured or painted";
