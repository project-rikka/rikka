use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt as _;
use rikka_llm::json;
use rikka_mcp::{Catalog, Http, Hub, Place, Server, Vault};
use rikka_test_keyring::TestKeyring;
use rikka_test_mcp::{Answering, Asked, Remote};
use tempfile::TempDir;

const NOTICING: Duration = Duration::from_secs(20);

async fn declared(remote: &Remote, vault: Option<Vault>) -> (TempDir, Hub) {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let hub = Hub::builder()
        .place(Place::new("assistant", folder.path().join("mcp.json")))
        .vault(vault)
        .build();
    let server = Server::new(
        "remote",
        Http::new(remote.url()).with_header("X-Team", "blue"),
    );
    hub.save_catalog("assistant", Catalog::from_servers([server]))
        .await
        .expect("file should be written");
    (folder, hub)
}

async fn connected(remote: &Remote) -> (TempDir, Hub) {
    let (folder, hub) = declared(remote, None).await;
    let standing = hub.status("remote").expect("server status should exist");
    assert!(standing.is_connected(), "{:?}", standing.error());
    (folder, hub)
}

async fn say(hub: &Hub) -> Result<String, String> {
    let kits = hub.toolkit_set().kits();
    let kit = kits
        .iter()
        .find(|kit| kit.name() == "remote")
        .expect("remote kit should be listed");
    let function = kit.functions().first().expect("kit should have a tool");
    function
        .call(json!({ "text": "hello" }))
        .await
        .map(|reply| reply.text().to_owned())
}

fn calls(remote: &Remote) -> Vec<Asked> {
    remote
        .asked()
        .into_iter()
        .filter(|asked| asked.rpc_method().as_deref() == Some("tools/call"))
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn json_answers_keep_session_and_start_another_when_forgotten() {
    let remote = Remote::start(Answering::Json);
    let (_folder, hub) = connected(&remote).await;

    assert_eq!(hub.status("remote").unwrap().title(), Some("Remote"));
    assert_eq!(say(&hub).await.as_deref(), Ok("say done"));

    let [call] = &calls(&remote)[..] else {
        panic!("one call should have been made");
    };
    assert_eq!(call.header("mcp-session-id"), Some("session-1"));
    assert_eq!(call.header("mcp-protocol-version"), Some("2025-11-25"));
    assert_eq!(call.header("x-team"), Some("blue"));
    assert_eq!(
        call.header("accept"),
        Some("application/json, text/event-stream")
    );
    assert_eq!(call.header("authorization"), None);

    remote.expire_session();
    assert_eq!(say(&hub).await.as_deref(), Ok("say done"));

    let sessions: Vec<Option<String>> = calls(&remote)
        .iter()
        .map(|call| call.header("mcp-session-id").map(str::to_owned))
        .collect();
    assert_eq!(
        sessions,
        ["session-1", "session-1", "session-2"].map(|session| Some(session.to_owned()))
    );
    let greetings = remote
        .rpc_methods()
        .iter()
        .filter(|method| *method == "initialize")
        .count();
    assert_eq!(greetings, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn event_stream_carries_answer_and_server_request() {
    let remote = Remote::start(Answering::Stream);
    let (_folder, hub) = connected(&remote).await;

    assert_eq!(say(&hub).await.as_deref(), Ok("say done"));

    let waiting = remote.clone();
    let pong = tokio::task::spawn_blocking(move || {
        waiting.expect(|asked| {
            asked
                .json()
                .is_some_and(|message| message["id"] == "are-you-there")
        })
    })
    .await
    .expect("the wait should finish");
    assert_eq!(
        pong.json(),
        Some(json!({ "jsonrpc": "2.0", "id": "are-you-there", "result": {} }))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn interrupted_stream_resumes_after_last_event() {
    let remote = Remote::start(Answering::Interrupted);
    let (_folder, hub) = connected(&remote).await;

    assert_eq!(say(&hub).await.as_deref(), Ok("say done"));

    let resumed: Vec<String> = remote
        .asked()
        .iter()
        .filter(|asked| asked.method() == "GET")
        .filter_map(|asked| asked.header("last-event-id").map(str::to_owned))
        .collect();
    assert_eq!(resumed, ["event-1"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn announced_tool_change_relists_over_standalone_stream() {
    let remote = Remote::start(Answering::Json);
    let (_folder, hub) = connected(&remote).await;
    let mut changes = std::pin::pin!(hub.changes());

    remote.offer("sing");

    let noticed = tokio::time::timeout(NOTICING, async {
        while hub.status("remote").unwrap().tool_count() < 2 {
            changes.next().await;
        }
    })
    .await;
    assert!(noticed.is_ok(), "the change was never noticed");
    let names: Vec<String> = hub
        .toolkit_set()
        .kits()
        .iter()
        .flat_map(|kit| kit.functions().iter().map(|tool| tool.name().to_owned()))
        .collect();
    assert_eq!(names, ["mcp__remote__say", "mcp__remote__sing"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn guarded_server_without_sign_in_is_locked() {
    let remote = Remote::guarded();
    let (_folder, hub) = declared(&remote, None).await;

    let standing = hub.status("remote").expect("server status should exist");
    assert!(standing.needs_sign_in(), "{:?}", standing.error());
    assert!(!hub.is_signed_in("remote"));
    assert!(
        hub.instruction().contains("remote"),
        "{}",
        hub.instruction()
    );
}

struct Browser {
    opened: PathBuf,
}

impl Browser {
    fn install(folder: &Path) -> Browser {
        let opened = folder.join("opened");
        let made = std::process::Command::new("mkfifo")
            .arg(&opened)
            .status()
            .expect("mkfifo should run");
        assert!(made.success());

        let programs = folder.join("bin");
        std::fs::create_dir(&programs).expect("program folder should be created");
        let opener = programs.join("rikka-open");
        std::fs::write(
            &opener,
            format!("#!/bin/sh\nprintf '%s' \"$1\" > '{}'\n", opened.display()),
        )
        .expect("opener should be written");
        std::fs::set_permissions(&opener, std::fs::Permissions::from_mode(0o755))
            .expect("opener should be executable");

        let mut searched = vec![programs];
        searched.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let path = std::env::join_paths(searched).expect("PATH should join");
        unsafe { std::env::set_var("PATH", path) };
        Browser { opened }
    }

    async fn follow(self) {
        let page = tokio::fs::read_to_string(&self.opened)
            .await
            .expect("a page should be opened");
        rikka_net::get(page.as_str())
            .await
            .expect("the page should load")
            .error_for_status()
            .await
            .expect("the sign-in should land on the callback");
    }
}

#[test]
fn sign_in_registers_client_and_renews_stale_token() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let browser = Browser::install(folder.path());
    let Some(_keyring) = TestKeyring::unlocked() else {
        return;
    };

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime should start")
        .block_on(async {
            let remote = Remote::guarded();
            let (_folder, hub) = declared(&remote, Vault::from_runtime_dir()).await;
            assert!(hub.status("remote").unwrap().needs_sign_in());

            let browsing = tokio::spawn(browser.follow());
            hub.sign_in("remote").await.expect("sign-in should succeed");
            browsing.await.expect("the browser should finish");

            let standing = hub.status("remote").expect("server status should exist");
            assert!(standing.is_connected(), "{:?}", standing.error());
            assert!(hub.is_signed_in("remote"));
            assert_eq!(say(&hub).await.as_deref(), Ok("say done"));

            let url = remote.url();
            let asked = remote.asked();
            let to = |path: &str| -> Vec<&Asked> {
                asked.iter().filter(|asked| asked.path() == path).collect()
            };

            let [registration] = to("/auth/register")[..] else {
                panic!("the client should register once");
            };
            let redirect = registration.json().expect("a JSON registration")["redirect_uris"][0]
                .as_str()
                .expect("a redirect URI")
                .to_owned();
            assert!(redirect.starts_with("http://localhost:"), "{redirect}");

            let [authorize] = to("/auth/authorize")[..] else {
                panic!("the sign-in page should open once");
            };
            assert_eq!(authorize.field("client_id").as_deref(), Some("registered"));
            assert_eq!(authorize.field("redirect_uri"), Some(redirect));
            assert_eq!(
                authorize.field("code_challenge_method").as_deref(),
                Some("S256")
            );
            assert_eq!(
                authorize.field("scope").as_deref(),
                Some("tools offline_access")
            );
            assert_eq!(authorize.field("resource").as_ref(), Some(&url));

            let [exchange, renewal] = to("/auth/token")[..] else {
                panic!("the code should be exchanged and the stale token renewed");
            };
            assert_eq!(
                exchange.field("grant_type").as_deref(),
                Some("authorization_code")
            );
            assert!(exchange.field("code_verifier").is_some());
            assert_eq!(exchange.field("resource").as_ref(), Some(&url));
            assert_eq!(
                renewal.field("grant_type").as_deref(),
                Some("refresh_token")
            );
            assert_eq!(renewal.field("refresh_token").as_deref(), Some("refresh-1"));
            assert_eq!(renewal.field("resource").as_ref(), Some(&url));

            let call = calls(&remote).pop().expect("the tool should be called");
            assert_eq!(call.header("authorization"), Some("Bearer access-2"));

            hub.sign_out("remote")
                .await
                .expect("sign-out should succeed");
            assert!(hub.status("remote").unwrap().needs_sign_in());
            assert!(!hub.is_signed_in("remote"));
        });
}
