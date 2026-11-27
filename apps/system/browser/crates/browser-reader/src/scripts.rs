use std::future::Future;

use rikka_web::{FrameResult, Run, Script, Session};
use serde_json::Value;

use crate::article::Article;
use crate::error::{Error, Result};
use crate::parse::{parse, readable};

const PROBE: Script = Script::bundled("browser-reader/probe.js");

const EXTRACT: Script = Script::bundled("browser-reader/extract.js");

pub fn probe(session: &Session) -> impl Future<Output = Result<bool>> + Send + use<> {
    let running = session.run(Run::load(PROBE).call("browser-reader.probe"));
    async move { readable(&top(running.await?)?) }
}

pub fn extract(session: &Session) -> impl Future<Output = Result<Article>> + Send + use<> {
    let running = session.run(Run::load(EXTRACT).call("browser-reader.extract"));
    async move { parse(&top(running.await?)?) }
}

fn top(results: Vec<FrameResult>) -> Result<Value> {
    results
        .into_iter()
        .next()
        .map(|result| result.value)
        .ok_or(Error::Malformed("frame result"))
}
