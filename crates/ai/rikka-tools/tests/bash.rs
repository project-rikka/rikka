use std::time::Duration;

use rikka_llm::json;
use rikka_tools::{BASH, JOB};

use crate::support::Project;

fn builds(project: &Project, command: &str) {
    project.holds("make", &format!("#!/usr/bin/env bash\n{command}\n"));
    let at = project.at("make");
    let mut how = std::fs::metadata(&at)
        .expect("file should exist")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut how, 0o755);
    std::fs::set_permissions(&at, how).expect("permissions should be set");
}

#[tokio::test]
async fn command_runs_in_project_and_returns_output() {
    let project = Project::open();
    project.holds("here.txt", "");

    let reply = project
        .call(BASH, json!({ "command": "ls" }))
        .await
        .expect("call input should parse");
    assert_eq!(reply.text(), "here.txt");
    assert!(!reply.is_failed());
    assert!(project.logged().is_empty(), "a short answer leaves no log");

    let failed = project
        .call(BASH, json!({ "command": "echo oh no >&2; exit 3" }))
        .await
        .expect("call input should parse");
    assert!(failed.is_failed(), "{}", failed.text());
    assert!(failed.text().contains("code 3"), "{}", failed.text());
    assert!(failed.text().contains("oh no"), "{}", failed.text());

    assert!(project.call(BASH, json!({ "timeout": 10 })).await.is_err());
}

#[tokio::test]
async fn build_command_returns_log_path() {
    let project = Project::open();
    builds(
        &project,
        "echo compiling; echo 'error[E0433]: no such thing' >&2; exit 2",
    );

    let reply = project
        .call(BASH, json!({ "command": "PATH=.:$PATH make -j8" }))
        .await
        .expect("call input should parse");

    assert!(reply.is_failed(), "{}", reply.text());
    assert!(reply.text().contains("code 2"), "{}", reply.text());
    assert!(!reply.text().contains("compiling"), "{}", reply.text());
    assert!(!reply.text().contains("E0433"), "{}", reply.text());
    let text = reply
        .text()
        .split_whitespace()
        .find(|word| word.starts_with(".rikka/tmp/"))
        .expect("reply should name the log");
    assert!(project.at(text).is_file(), "{text}");
    assert!(project.at(".rikka/tmp/.gitignore").is_file());
    let log = project.only_log();
    assert!(log.contains("compiling"), "{log}");
    assert!(log.contains("E0433"), "{log}");
}

#[tokio::test]
async fn long_output_is_saved_to_log() {
    let project = Project::open();

    let reply = project
        .call(BASH, json!({ "command": "seq 1 20000" }))
        .await
        .expect("call input should parse");

    assert!(reply.text().starts_with("1\n2\n"), "{}", reply.text());
    assert!(reply.text().contains("truncated"), "{}", reply.text());
    assert!(reply.text().contains(".rikka/tmp/"), "{}", reply.text());
    assert!(project.only_log().ends_with("20000\n"));
}

#[tokio::test]
async fn background_command_returns_job() {
    let project = Project::open();
    let jobs = project.toolbox().jobs();

    let reply = project
        .call(
            BASH,
            json!({ "command": "sleep 0.2; echo later", "background": true }),
        )
        .await
        .expect("call input should parse");
    assert!(reply.text().contains("job 1"), "{}", reply.text());
    assert_eq!(jobs.running(), 1);
    assert!(jobs.notices().take().is_empty());

    let arrived = jobs.notices().wait().await;
    assert_eq!(arrived.len(), 1, "{arrived:?}");
    assert!(arrived[0].text().contains("exited 0"), "{arrived:?}");

    let listed = project
        .call(JOB, json!({ "action": "list" }))
        .await
        .expect("job list should succeed");
    assert!(listed.text().contains("sleep 0.2"), "{}", listed.text());
    let read = project
        .call(JOB, json!({ "action": "output", "id": 1 }))
        .await
        .expect("file should be readable");
    assert!(read.text().contains("later"), "{}", read.text());
    let again = project
        .call(JOB, json!({ "action": "output", "id": 1 }))
        .await
        .expect("file should be readable");
    assert!(!again.text().contains("later"), "{}", again.text());
    assert!(project.only_log().contains("later"));
    for refused in [json!({ "action": "output" }), json!({ "action": "nap" })] {
        assert!(project.call(JOB, refused).await.is_err());
    }
}

#[tokio::test]
async fn timed_out_command_moves_to_background() {
    let project = Project::open();
    let jobs = project.toolbox().jobs();

    let reply = project
        .call(
            BASH,
            json!({ "command": "echo starting; sleep 0.4; echo ending", "timeout": 100 }),
        )
        .await
        .expect("call input should parse");

    assert!(
        reply.text().contains("moved to the background"),
        "{}",
        reply.text()
    );
    assert!(reply.text().contains("starting"), "{}", reply.text());
    assert_eq!(jobs.running(), 1);
    let arrived = jobs.notices().wait().await;
    assert!(arrived[0].text().contains("exited 0"), "{arrived:?}");
}

#[tokio::test]
async fn stop_by_the_person_is_held_for_the_next_turn() {
    let project = Project::open();
    let jobs = project.toolbox().jobs();
    let background = json!({ "command": "echo starting; sleep 30", "timeout": 100 });

    for by_the_person in [true, false] {
        project
            .call(BASH, background.clone())
            .await
            .expect("call input should parse");
        let id = jobs.snapshot().last().expect("job should be listed").id();
        let stopped = match by_the_person {
            true => jobs.interrupt(id),
            false => jobs.stop(id),
        };
        assert!(stopped);
        let arrived = jobs.notices().wait().await;

        match by_the_person {
            true => {
                assert!(
                    arrived[0].text().contains("stopped by the person"),
                    "{arrived:?}"
                );
                assert!(!arrived[0].wakes(), "{arrived:?}");
            }
            false => assert!(arrived.is_empty(), "{arrived:?}"),
        }
    }
}

#[tokio::test]
async fn idle_command_is_killed_at_timeout() {
    let project = Project::open();

    let reply = project
        .call(BASH, json!({ "command": "sleep 30", "timeout": 100 }))
        .await
        .expect("call input should parse");

    assert!(reply.is_failed(), "{}", reply.text());
    assert!(reply.text().contains("killed"), "{}", reply.text());
    assert_eq!(project.toolbox().jobs().running(), 0);
}

#[tokio::test]
async fn dropped_call_kills_command() {
    let project = Project::open();
    let later = project.at("later.txt");
    let command = format!("sleep 1; echo later > {}", later.display());

    let outcome = tokio::time::timeout(
        Duration::from_millis(200),
        project.call(BASH, json!({ "command": command })),
    )
    .await;

    assert!(
        outcome.is_err(),
        "the command ended before it was let go of"
    );
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert!(
        !later.exists(),
        "the command carried on after the call ended"
    );
    assert_eq!(project.toolbox().jobs().running(), 0);
}
