use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rikka_assist::{Apps, Offering, ToolRegistration};
use rikka_tool::{Function, Tool, ToolKit, ToolKitSet, Value, json};

const PATIENCE: Duration = Duration::from_secs(10);

fn posing(window: &'static str) -> Function {
    let tool = Tool::new("Pose")
        .description("Keys a pose")
        .string("figure", "Whom to pose")
        .build()
        .expect("tool should be valid");
    Function::new(tool, move |input: Value| async move {
        let figure = input["figure"].as_str().unwrap_or("nobody").to_owned();
        let window_given = input.get("window").is_some();
        Ok::<_, String>(format!(
            "{window} posed {figure}, window given: {window_given}"
        ))
    })
}

struct Dropped(Arc<AtomicBool>);

impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn named(name: &str) -> Function {
    let tool = Tool::new(name)
        .description("Does nothing")
        .build()
        .expect("tool should be valid");
    Function::new(tool, |_: Value| async { Ok::<_, String>("done") })
}

fn waiting(stopped: Arc<AtomicBool>) -> Function {
    let tool = Tool::new("Wait")
        .description("Never finishes")
        .build()
        .expect("tool should be valid");
    Function::new(tool, move |_: Value| {
        let guard = Dropped(Arc::clone(&stopped));
        async move {
            std::future::pending::<()>().await;
            drop(guard);
            Ok::<_, String>("never")
        }
    })
}

fn until<T>(what: &str, mut ready: impl FnMut() -> Option<T>) -> T {
    let started = Instant::now();
    loop {
        if let Some(ready) = ready() {
            return ready;
        }
        assert!(started.elapsed() < PATIENCE, "{what} never happened");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn kit(shelf: &ToolKitSet, tools: usize) -> Option<ToolKit> {
    shelf
        .kits()
        .iter()
        .find(|kit| kit.name() == "Animate")
        .cloned()
        .filter(|kit| kit.functions().len() == tools)
}

fn function(kit: &ToolKit, name: &str) -> Function {
    kit.functions()
        .iter()
        .find(|function| function.name() == name)
        .unwrap_or_else(|| panic!("{name} is offered"))
        .clone()
}

fn call_tool(working: &tokio::runtime::Runtime, function: &Function, input: Value) -> String {
    working
        .block_on(function.call(input))
        .expect("call should succeed")
        .text()
        .to_owned()
}

fn session(folder: &Path) {
    unsafe {
        std::env::set_var("RIKKA_SEAM_SOCKET", folder.join("seam.sock"));
    }
}

#[test]
fn app_tools_target_last_used_window() {
    let folder = tempfile::tempdir().expect("session dir should be created");
    session(folder.path());
    let working = tokio::runtime::Runtime::new().expect("tokio runtime should build");

    let shelf = ToolKitSet::new();
    let apps = Apps::watch_dir(shelf.clone(), &folder.path().join("assist"));
    let offering = || {
        Offering::new("Animate")
            .expect("kit name should be valid")
            .with_title("アニメート")
            .with_description("Character animation")
            .with_instructions("Read the scene first.")
            .gist("Pose", ["figure"])
    };
    let stopped = Arc::new(AtomicBool::new(false));

    let dance = ToolRegistration::register(
        offering(),
        "Dance",
        vec![posing("dance"), waiting(Arc::clone(&stopped))],
    )
    .expect("tools should be offered");
    let walk = ToolRegistration::register(
        offering(),
        "Walk",
        vec![posing("walk"), waiting(Arc::clone(&stopped))],
    )
    .expect("app should offer another window");
    assert!(
        ToolRegistration::register(
            Offering::new("Paint").expect("kit name should be valid"),
            "Picture",
            Vec::new()
        )
        .is_err(),
        "one process offers one app"
    );
    assert!(
        ToolRegistration::register(offering(), "Odd", vec![named("Windows")]).is_err(),
        "the window listing keeps its name"
    );

    let kit_entry = until("the app arriving", || kit(&shelf, 3));
    assert_eq!(kit_entry.instructions(), "Read the scene first.");
    assert_eq!(kit_entry.description(), "Character animation");
    let pose = function(&kit_entry, "Animate_Pose");
    assert!(
        pose.tool().schema()["properties"]["window"].is_object(),
        "{}",
        pose.tool().schema()
    );

    assert_eq!(
        call_tool(&working, &pose, json!({ "figure": "Miku" })),
        "walk posed Miku, window given: false"
    );
    dance.mark_active();
    assert_eq!(
        call_tool(&working, &pose, json!({ "figure": "Miku" })),
        "dance posed Miku, window given: false"
    );
    assert_eq!(
        call_tool(
            &working,
            &pose,
            json!({ "figure": "Rin", "window": "Walk" })
        ),
        "walk posed Rin, window given: false"
    );
    assert_eq!(
        call_tool(
            &working,
            &function(&kit_entry, "Animate_Windows"),
            json!({})
        ),
        "- Dance\n- Walk"
    );
    let elsewhere = working
        .block_on(pose.call(json!({ "figure": "Rin", "window": "Sing" })))
        .expect("call should succeed");
    assert!(elsewhere.is_failed());

    let running = apps.running();
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].kit(), "Animate");
    assert_eq!(running[0].title(), "アニメート");
    assert_eq!(running[0].about(), "Character animation");
    let used = apps
        .used_tool("Animate_Pose")
        .expect("calls should be shown");
    assert_eq!(used.app(), "アニメート");
    assert_eq!(used.tool(), "Pose");
    assert_eq!(
        used.gist(&json!({ "figure": "Miku" })),
        Some("Miku".to_owned())
    );

    let wait = function(&kit_entry, "Animate_Wait");
    let gave_up = working.block_on(async {
        tokio::time::timeout(Duration::from_millis(200), wait.call(json!({}))).await
    });
    assert!(gave_up.is_err(), "the call never finishes by itself");
    until("the abandoned call stopping in the app", || {
        stopped.load(Ordering::Acquire).then_some(())
    });

    drop(dance);
    drop(walk);
    until("the app's tools leaving with its last window", || {
        shelf.kits().is_empty().then_some(())
    });
    assert!(apps.running().is_empty());
    assert!(
        apps.used_tool("Animate_Pose").is_some(),
        "calls already made still show"
    );

    let left = std::fs::read_dir(folder.path().join("assist"))
        .expect("directory should be readable")
        .flatten()
        .count();
    assert_eq!(left, 0, "the app's socket goes with its last window");
}
