use futures_util::future::BoxFuture;

use crate::call::{Call, Translated};
use crate::error::Result;
use crate::http::Http;
use crate::service::Service;

pub type Translating<'a> = BoxFuture<'a, Result<Vec<Translated>>>;

pub trait Adapter: Send + Sync + 'static {
    fn service(&self) -> &'static Service;

    fn translate<'a>(&'a self, http: &'a Http, call: Call<'a>) -> Translating<'a>;
}
