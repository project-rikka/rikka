use std::future::Future;

use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use rikka_llm::wire::ListedModel;
use rikka_llm_catalog::{Catalog, Lookup, lineup};

use crate::adapter::Models;

pub struct Listing {
    lookup: Lookup,
    asked: BoxFuture<'static, rikka_llm::Result<Vec<ListedModel>>>,
}

impl Listing {
    pub fn new(
        lookup: Lookup,
        asked: impl Future<Output = rikka_llm::Result<Vec<ListedModel>>> + Send + 'static,
    ) -> Listing {
        Listing {
            lookup,
            asked: asked.boxed(),
        }
    }

    pub(crate) fn described(self) -> Models {
        async move {
            let (listed, catalog) = futures_util::try_join!(self.asked, Catalog::current())?;
            Ok(lineup(listed, &catalog, self.lookup))
        }
        .boxed()
    }
}

impl std::fmt::Debug for Listing {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Listing")
            .field("lookup", &self.lookup)
            .finish_non_exhaustive()
    }
}
