use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

use crate::error::Result;
use crate::request::Request;
use crate::response::Response;
use crate::stream::Stream;
use crate::vendor::Vendor;

pub type Call<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

pub(crate) trait Provider: Debug + Send + Sync + 'static {
    fn vendor(&self) -> Vendor;

    fn model(&self) -> &str;

    fn send<'a>(&'a self, request: &'a Request) -> Call<'a, Response>;

    fn stream<'a>(&'a self, request: &'a Request) -> Call<'a, Stream>;
}
