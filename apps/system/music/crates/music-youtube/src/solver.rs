use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, mpsc};

use music_core::{Error, Result};
use music_stream::{Scan, answer_text, api_client};
use rikka_net::Client as Http;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use tokio::sync::{Mutex, oneshot};

use crate::clients::Client;

const PRELUDE: &str = include_str!("../js/prelude.js");
const MERIYAH: &str = include_str!("../js/meriyah.js");
const ASTRING: &str = include_str!("../js/astring.js");
const SOLVER: &str = include_str!("../js/solver.js");
const IFRAME_API: &str = "https://www.youtube.com/iframe_api";
const CACHE_FILE: &str = "solver.json";
const PLAYER_ID_LEN: usize = 8;
const SOLVER_THREAD: &str = "rikka-youtube-js";
const JS_STACK: usize = 16 * 1024 * 1024;
const THREAD_STACK: usize = 64 * 1024 * 1024;
const MEMORY: usize = 512 * 1024 * 1024;

const PREPROCESS: &str = r#"
var __prepared = jsc({
  type: "player",
  player: __player,
  requests: [],
  output_preprocessed: true,
});
__player = null;
var __code = __prepared.preprocessed_player;
__prepared = null;
__code
"#;

const INSTALL: &str = r#"
globalThis.__solvers = { sig: null, n: null };
Function("_result", __solver)(__solvers);
__solver = null;
if (!__solvers.sig && !__solvers.n) {
  throw "the player script yields no solver";
}
"#;

const RUN: &str = r#"
(function () {
  try {
    return JSON.stringify({
      sig: __sig === null || __sig === undefined ? null : __solvers.sig(__sig),
      n: __n === null || __n === undefined ? null : __solvers.n(__n),
    });
  } catch (error) {
    return JSON.stringify({ error: error instanceof Error ? error.message : String(error) });
  }
})()
"#;

#[derive(Debug)]
pub(crate) struct Solving {
    http: Http,
    cache: PathBuf,
    ready: Mutex<Option<Arc<Solver>>>,
}

impl Solving {
    pub(crate) fn new(cache: PathBuf) -> Solving {
        Solving {
            http: api_client(Client::Music.agent()),
            cache,
            ready: Mutex::new(None),
        }
    }

    pub(crate) async fn solver(&self) -> Result<Arc<Solver>> {
        let mut ready = self.ready.lock().await;
        if let Some(solver) = ready.as_ref() {
            return Ok(Arc::clone(solver));
        }
        let cache_file = self.cache.join(CACHE_FILE);
        let script = self.script(&cache_file).await?;
        let solver = Arc::new(Solver::start(script, cache_file).await?);
        *ready = Some(Arc::clone(&solver));
        Ok(solver)
    }
    pub(crate) async fn outdated(&self, solver: &Arc<Solver>) {
        let mut ready = self.ready.lock().await;
        if ready
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, solver))
        {
            *ready = None;
        }
    }

    async fn script(&self, cache_file: &Path) -> Result<Script> {
        let page = answer_text(self.http.get(IFRAME_API)).await?;
        let id = player_id(&page)
            .ok_or_else(|| Error::Unreadable("the iframe API has no player".into()))?;
        if let Some(known) = load_cached(cache_file, &id) {
            return Ok(Script {
                id: known.id,
                sts: known.sts,
                code: known.code,
                prepared: true,
            });
        }
        let code = answer_text(self.http.get(format!(
            "https://www.youtube.com/s/player/{id}/player_ias.vflset/en_US/base.js"
        )))
        .await?;
        let sts = signature_timestamp(&code)
            .ok_or_else(|| Error::Unreadable(format!("player {id} has no signature timestamp")))?;
        Ok(Script {
            id,
            sts,
            code,
            prepared: false,
        })
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Solved {
    pub(crate) signature: Option<String>,
    pub(crate) throttle: Option<String>,
}

struct Job {
    signature: Option<String>,
    throttle: Option<String>,
    reply: oneshot::Sender<Result<Solved>>,
}

#[derive(Debug)]
pub(crate) struct Solver {
    sts: u32,
    jobs: mpsc::Sender<Job>,
}

struct Script {
    id: String,
    sts: u32,
    code: String,
    prepared: bool,
}

#[derive(Serialize, Deserialize)]
struct CachedSolver {
    id: String,
    sts: u32,
    bundle: String,
    code: String,
}

impl Solver {
    async fn start(script: Script, cache_file: PathBuf) -> Result<Solver> {
        let sts = script.sts;
        let (jobs, queue) = mpsc::channel();
        let (ready, started) = oneshot::channel();
        std::thread::Builder::new()
            .name(SOLVER_THREAD.into())
            .stack_size(THREAD_STACK)
            .spawn(move || serve(&script, &cache_file, &queue, ready))
            .map_err(|error| Error::Spawn(format!("{SOLVER_THREAD}: {error}")))?;
        started
            .await
            .map_err(|_| Error::Unreadable("the player solver stopped while starting".into()))??;
        Ok(Solver { sts, jobs })
    }

    pub(crate) fn sts(&self) -> u32 {
        self.sts
    }

    pub(crate) async fn solve(
        &self,
        signature: Option<&str>,
        throttle: Option<&str>,
    ) -> Result<Solved> {
        if signature.is_none() && throttle.is_none() {
            return Ok(Solved::default());
        }
        let (reply, answer) = oneshot::channel();
        self.jobs
            .send(Job {
                signature: signature.map(str::to_owned),
                throttle: throttle.map(str::to_owned),
                reply,
            })
            .map_err(|_| Error::Unreadable("the player solver has stopped".into()))?;
        answer
            .await
            .map_err(|_| Error::Unreadable("the player solver dropped a request".into()))?
    }
}

fn serve(
    script: &Script,
    cache_file: &Path,
    queue: &mpsc::Receiver<Job>,
    ready: oneshot::Sender<Result<()>>,
) {
    let context = match prepare(script, cache_file) {
        Ok(context) => context,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        return;
    }
    while let Ok(job) = queue.recv() {
        let answered = answer(&context, job.signature.as_deref(), job.throttle.as_deref());
        let _ = job.reply.send(answered);
    }
}

fn prepare(script: &Script, cache_file: &Path) -> Result<rquickjs::Context> {
    let runtime = rquickjs::Runtime::new().map_err(unreadable)?;
    runtime.set_max_stack_size(JS_STACK);
    runtime.set_memory_limit(MEMORY);
    let context = rquickjs::Context::full(&runtime).map_err(unreadable)?;
    context.with(|ctx| -> Result<()> {
        let sources: &[&str] = match script.prepared {
            true => &[PRELUDE],
            false => &[PRELUDE, MERIYAH, ASTRING, SOLVER],
        };
        for source in sources {
            ctx.eval::<(), _>(*source)
                .map_err(|error| exception_error(&ctx, error))?;
        }
        let solver = match script.prepared {
            true => script.code.clone(),
            false => {
                ctx.globals()
                    .set("__player", script.code.as_str())
                    .map_err(|error| exception_error(&ctx, error))?;
                let solver: String = ctx
                    .eval(PREPROCESS)
                    .map_err(|error| exception_error(&ctx, error))?;
                store_cached(cache_file, script, &solver);
                solver
            }
        };
        ctx.globals()
            .set("__solver", solver)
            .map_err(|error| exception_error(&ctx, error))?;
        ctx.eval::<(), _>(INSTALL)
            .map_err(|error| exception_error(&ctx, error))
    })?;
    runtime.run_gc();
    Ok(context)
}

fn answer(
    context: &rquickjs::Context,
    signature: Option<&str>,
    throttle: Option<&str>,
) -> Result<Solved> {
    let output: String = context.with(|ctx| -> Result<String> {
        ctx.globals()
            .set("__sig", signature)
            .map_err(|error| exception_error(&ctx, error))?;
        ctx.globals()
            .set("__n", throttle)
            .map_err(|error| exception_error(&ctx, error))?;
        ctx.eval(RUN).map_err(|error| exception_error(&ctx, error))
    })?;
    let parsed: Value = serde_json::from_str(&output).map_err(unreadable)?;
    if let Some(error) = parsed.get("error").and_then(Value::as_str) {
        return Err(Error::Unreadable(format!(
            "the player solver failed: {}",
            error.lines().next().unwrap_or(error)
        )));
    }
    let solved = |key: &str, requested: bool| -> Result<Option<String>> {
        match requested {
            false => Ok(None),
            true => parsed
                .get(key)
                .and_then(Value::as_str)
                .map(|value| Some(value.to_owned()))
                .ok_or_else(|| Error::Unreadable(format!("the player solver returned no {key}"))),
        }
    };
    Ok(Solved {
        signature: solved("sig", signature.is_some())?,
        throttle: solved("n", throttle.is_some())?,
    })
}

static BUNDLE: LazyLock<String> = LazyLock::new(|| {
    let mut hasher = Sha1::new();
    for source in [PRELUDE, MERIYAH, ASTRING, SOLVER] {
        hasher.update(source.as_bytes());
    }
    rikka_types::hex::encode(&hasher.finalize()[..4])
});

fn load_cached(cache_file: &Path, id: &str) -> Option<CachedSolver> {
    let known: CachedSolver = serde_json::from_slice(&std::fs::read(cache_file).ok()?).ok()?;
    (known.id == id && known.bundle == *BUNDLE).then_some(known)
}

fn store_cached(cache_file: &Path, script: &Script, solver: &str) {
    let known = CachedSolver {
        id: script.id.clone(),
        sts: script.sts,
        bundle: BUNDLE.clone(),
        code: solver.to_owned(),
    };
    let stored = cache_file
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .map_err(unreadable)
        .and_then(|()| Ok(rikka_account_book::write_private_json(cache_file, &known)?));
    if let Err(error) = stored {
        tracing::warn!("failed to cache YouTube player: {error}");
    }
}

fn player_id(page: &str) -> Option<String> {
    Scan::new(page)
        .matches("player", |mut scan| {
            scan.skip("\\");
            scan.expect("/")?;
            let id = scan.take(PLAYER_ID_LEN..=PLAYER_ID_LEN, |c| c.is_ascii_hexdigit())?;
            scan.skip("\\");
            scan.expect("/")?;
            Some(id.to_owned())
        })
        .next()
}

fn signature_timestamp(code: &str) -> Option<u32> {
    Scan::new(code)
        .matches("signatureTimestamp:", |mut scan| {
            scan.take(1..=usize::MAX, |c| c.is_ascii_digit())?
                .parse()
                .ok()
        })
        .next()
}

fn unreadable(error: impl std::fmt::Display) -> Error {
    Error::Unreadable(error.to_string())
}

fn exception_error(ctx: &rquickjs::Ctx<'_>, error: rquickjs::Error) -> Error {
    if !matches!(error, rquickjs::Error::Exception) {
        return unreadable(error);
    }
    let caught = ctx.catch();
    let message = caught
        .as_exception()
        .and_then(rquickjs::Exception::message)
        .or_else(|| caught.as_string().and_then(|text| text.to_string().ok()))
        .unwrap_or_else(|| "the player script threw something unreadable".into());
    Error::Unreadable(message)
}
