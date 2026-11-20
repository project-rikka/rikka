use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use files_core::Location;
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use ignore::{WalkBuilder, WalkState};

use crate::query::{Hit, Options, Query};

pub type Sink = Arc<dyn Fn(Hit) + Send + Sync>;

pub type Finished = Arc<dyn Fn(Completion) + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    Exhausted,
    Truncated,
    Stopped,
}

#[derive(Debug)]
pub struct Search {
    stop: Arc<AtomicBool>,
}

impl Search {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Search {
    fn drop(&mut self) {
        self.stop();
    }
}

#[must_use]
pub fn start(root: &Location, query: &Query, sink: Sink, finished: Finished) -> Search {
    let stop = Arc::new(AtomicBool::new(false));
    let search = Search {
        stop: Arc::clone(&stop),
    };

    let Some(directory) = root.to_path() else {
        finished(Completion::Exhausted);
        return search;
    };
    if query.is_empty() {
        finished(Completion::Exhausted);
        return search;
    }

    let name_matcher = if query.name.trim().is_empty() {
        None
    } else {
        matcher(&query.name, query.options).ok()
    };
    let content_matcher = query
        .reads_contents()
        .then(|| matcher(&query.contents, query.options).ok())
        .flatten();

    if (!query.name.trim().is_empty() && name_matcher.is_none())
        || (query.reads_contents() && content_matcher.is_none())
    {
        finished(Completion::Exhausted);
        return search;
    }

    let options = query.options;
    let matched_count = Arc::new(AtomicUsize::new(0));
    let walker_stop = Arc::clone(&stop);

    std::thread::Builder::new()
        .name("rikka-files-find".to_owned())
        .spawn(move || {
            let mut builder = WalkBuilder::new(&directory);
            builder
                .hidden(!options.hidden)
                .git_ignore(options.respect_ignore_files)
                .git_global(options.respect_ignore_files)
                .git_exclude(options.respect_ignore_files)
                .ignore(options.respect_ignore_files)
                .parents(options.respect_ignore_files)
                .require_git(false)
                .follow_links(false)
                .threads(walkers());

            builder.build_parallel().run(|| {
                let sink = Arc::clone(&sink);
                let stop = Arc::clone(&walker_stop);
                let matched_count = Arc::clone(&matched_count);
                let name_matcher = name_matcher.clone();
                let content_matcher = content_matcher.clone();
                let mut searcher = SearcherBuilder::new()
                    .line_number(true)
                    .binary_detection(BinaryDetection::quit(0))
                    .build();
                Box::new(move |entry| {
                    if stop.load(Ordering::Relaxed)
                        || matched_count.load(Ordering::Relaxed) >= options.limit
                    {
                        return WalkState::Quit;
                    }
                    let Ok(entry) = entry else {
                        return WalkState::Continue;
                    };
                    let path = entry.path();
                    if entry.depth() == 0 {
                        return WalkState::Continue;
                    }
                    let name = entry.file_name().to_string_lossy();
                    if let Some(matcher) = &name_matcher
                        && !matches_name(matcher, &name)
                    {
                        return WalkState::Continue;
                    }

                    let hit = match &content_matcher {
                        None => Some(Hit {
                            at: Location::from_path(path),
                            line: None,
                            line_number: None,
                        }),
                        Some(matcher) => {
                            if entry.file_type().is_none_or(|kind| !kind.is_file()) {
                                return WalkState::Continue;
                            }
                            first_match(&mut searcher, matcher, path)
                        }
                    };
                    if let Some(hit) = hit {
                        if matched_count.fetch_add(1, Ordering::Relaxed) >= options.limit {
                            return WalkState::Quit;
                        }
                        sink(hit);
                    }
                    WalkState::Continue
                })
            });

            let outcome = if walker_stop.load(Ordering::Relaxed) {
                Completion::Stopped
            } else if matched_count.load(Ordering::Relaxed) >= options.limit {
                Completion::Truncated
            } else {
                Completion::Exhausted
            };
            finished(outcome);
        })
        .ok();

    search
}

fn walkers() -> usize {
    std::thread::available_parallelism()
        .map(|cores| cores.get().saturating_sub(2).clamp(1, 8))
        .unwrap_or(1)
}

fn matcher(pattern: &str, options: Options) -> Result<RegexMatcher, grep_regex::Error> {
    grep_regex::RegexMatcherBuilder::new()
        .case_insensitive(!options.case_sensitive)
        .fixed_strings(!options.regex)
        .line_terminator(Some(b'\n'))
        .build(pattern.trim())
}

fn matches_name(matcher: &RegexMatcher, name: &str) -> bool {
    use grep_matcher::Matcher;
    matcher.is_match(name.as_bytes()).unwrap_or(false)
}

fn first_match(
    searcher: &mut grep_searcher::Searcher,
    matcher: &RegexMatcher,
    path: &std::path::Path,
) -> Option<Hit> {
    let mut hit = None;
    searcher
        .search_path(
            matcher,
            path,
            UTF8(|number, line| {
                hit = Some((number, line.trim_end().to_owned()));
                Ok(false)
            }),
        )
        .ok()?;

    hit.map(|(number, line)| Hit {
        at: Location::from_path(path),
        line: Some(line),
        line_number: Some(number),
    })
}
