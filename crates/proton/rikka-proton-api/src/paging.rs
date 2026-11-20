use futures_util::{Stream, TryStreamExt as _, stream};
use serde::de::DeserializeOwned;

use crate::error::Result;
use crate::request::RequestBuilder;

const PAGE: &str = "Page";
const PAGE_SIZE: &str = "PageSize";
const SINCE: &str = "Since";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paging {
    position: Position,
    size: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Position {
    Numbered {
        page: u32,
    },
    Cursor {
        name: &'static str,
        at: Option<String>,
    },
}

impl Paging {
    pub fn numbered(size: u32) -> Paging {
        Paging {
            position: Position::Numbered { page: 0 },
            size: Some(size.max(1)),
        }
    }

    pub fn cursor(name: &'static str) -> Paging {
        Paging {
            position: Position::Cursor { name, at: None },
            size: None,
        }
    }

    pub fn since() -> Paging {
        Paging::cursor(SINCE)
    }

    #[must_use]
    pub fn starting_at(mut self, first: u32) -> Paging {
        if let Position::Numbered { page } = &mut self.position {
            *page = first;
        }
        self
    }

    #[must_use]
    pub fn starting_from(mut self, cursor: impl ToString) -> Paging {
        if let Position::Cursor { at, .. } = &mut self.position {
            *at = Some(cursor.to_string());
        }
        self
    }

    #[must_use]
    pub fn page_size(mut self, size: u32) -> Paging {
        self.size = Some(size.max(1));
        self
    }

    fn is_full(&self, count: usize) -> bool {
        self.size.is_none_or(|size| count >= size as usize)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    items: Vec<T>,
    total: Option<usize>,
    next: Option<String>,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>) -> Page<T> {
        Page {
            items,
            total: None,
            next: None,
        }
    }

    #[must_use]
    pub fn total(mut self, total: Option<usize>) -> Page<T> {
        self.total = total;
        self
    }

    #[must_use]
    pub fn next(mut self, cursor: Option<impl ToString>) -> Page<T> {
        self.next = cursor
            .map(|cursor| cursor.to_string())
            .filter(|cursor| !cursor.is_empty());
        self
    }
}

#[must_use]
#[derive(Debug)]
pub struct Pages {
    request: RequestBuilder,
    paging: Paging,
    field: Vec<&'static str>,
}

impl Pages {
    pub(crate) fn new(request: RequestBuilder, paging: Paging) -> Pages {
        Pages {
            request,
            paging,
            field: Vec::new(),
        }
    }

    pub fn field(self, name: &'static str) -> Pages {
        self.field_at(&[name])
    }

    pub fn field_at(mut self, path: &[&'static str]) -> Pages {
        self.field = path.to_vec();
        self
    }

    pub async fn collect<A, T>(self, extract: impl FnMut(A) -> Page<T>) -> Result<Vec<T>>
    where
        A: DeserializeOwned,
    {
        self.stream(extract).try_concat().await
    }

    pub fn stream<A, T, F>(self, extract: F) -> impl Stream<Item = Result<Vec<T>>>
    where
        A: DeserializeOwned,
        F: FnMut(A) -> Page<T>,
    {
        let Pages {
            request,
            paging,
            field,
        } = self;
        let first = request.into_validated().map(|template| PageStream {
            template,
            paging,
            field,
            fetched: 0,
            extract,
        });

        stream::try_unfold(Some(first), |state| async move {
            let Some(pager) = state else {
                return Ok(None);
            };
            let mut pager = pager?;
            let (items, more) = pager.step().await?;
            Ok(Some((items, more.then_some(Ok(pager)))))
        })
    }
}

struct PageStream<F> {
    template: RequestBuilder,
    paging: Paging,
    field: Vec<&'static str>,
    fetched: usize,
    extract: F,
}

impl<F> PageStream<F> {
    async fn step<A, T>(&mut self) -> Result<(Vec<T>, bool)>
    where
        A: DeserializeOwned,
        F: FnMut(A) -> Page<T>,
    {
        let mut page_request = self.template.replicate();
        match &self.paging.position {
            Position::Numbered { page } => {
                page_request = page_request.query(&[(PAGE, page.to_string())]);
            }
            Position::Cursor { name, at: Some(at) } => {
                page_request = page_request.query(&[(*name, at)]);
            }
            Position::Cursor { at: None, .. } => {}
        }
        if let Some(size) = self.paging.size {
            page_request = page_request.query(&[(PAGE_SIZE, size.to_string())]);
        }

        let answer: Option<A> = match self.field.is_empty() {
            true => Some(page_request.send().await?),
            false => page_request.send_field_at(&self.field).await?,
        };
        let Page { items, total, next } = match answer {
            Some(answer) => (self.extract)(answer),
            None => Page::new(Vec::new()),
        };
        let full = self.paging.is_full(items.len());
        self.fetched += items.len();

        let more = match &mut self.paging.position {
            Position::Numbered { page } => {
                let complete = total.is_some_and(|total| self.fetched >= total);
                *page += 1;
                full && !complete
            }
            Position::Cursor { at, .. } => match next {
                Some(next) if full && Some(&next) != at.as_ref() => {
                    *at = Some(next);
                    true
                }
                _ => false,
            },
        };
        Ok((items, more))
    }
}
