use rikka_llm::json;
use rikka_tools::{ASK, BASH, COMPUTER, EDIT, TOOL_SEARCH, VIEW, WRITE};

use crate::support::Project;

const REFUSED: &str = "only commands that read, search or list run here";

#[tokio::test]
async fn read_only_toolbox_has_no_writers() {
    let project = Project::fitted(|setup| setup.read_only());

    for stem in [COMPUTER, WRITE, EDIT, TOOL_SEARCH] {
        assert!(!project.declares(stem), "{stem}");
    }
    for stem in [BASH, VIEW, ASK] {
        assert!(project.declares(stem), "{stem}");
    }
}

#[tokio::test]
async fn read_only_shell_allows_readers() {
    let project = Project::fitted(|setup| setup.read_only());
    project.holds("notes.txt", "kept as it was\n");
    project.holds("src/a.rs", "fn a() {}\n");

    for command in [
        "cat notes.txt",
        "head -n 40 notes.txt src/a.rs",
        "sed -n '1,80p' notes.txt",
        r#"grep -rn "fn a" src"#,
        "rg --glob '*.rs' -t rust fn",
        "find src -type f -name '*.rs'",
        "ls -la src",
        "cat notes.txt | grep kept",
        "cd src && cat a.rs",
        "find . -name '*.rs' 2>/dev/null",
        "grep -rn todo src 2>&1 | head -20",
        "ls src > /dev/null",
        r"find src -name '*.rs' -exec grep -l fn {} +",
        r#"cat "notes.txt""#,
    ] {
        let reply = project.call(BASH, json!({ "command": command })).await;
        assert!(
            !matches!(&reply, Err(refused) if refused.contains(REFUSED)),
            "{command}: {reply:?}"
        );
    }
}

#[tokio::test]
async fn read_only_shell_rejects_writers() {
    let project = Project::fitted(|setup| setup.read_only());
    project.holds("notes.txt", "kept as it was\n");

    for command in [
        "rm notes.txt",
        "cat notes.txt > copy.txt",
        "cat notes.txt >> copy.txt",
        "cargo build 2>&1 | grep error",
        "grep -rn todo . | wc -l",
        "sed -i 's/kept/lost/' notes.txt",
        "sed -n 's/a/b/w copy.txt' notes.txt",
        "find . -name '*.txt' -delete",
        r"find . -name '*.txt' -exec rm {} \;",
        r"find . -exec find {} -exec cat {} \; \;",
        "fd -e txt -x rm",
        "fd --exec=rm cat",
        "tree -o copy.txt",
        "bat --paging=always --pager rm notes.txt",
        "bat cache --clear",
        "{ cat notes.txt; ls; }",
        "(cat notes.txt)",
        "cat notes.txt &",
        "cat $(echo notes.txt)",
        "cat `echo notes.txt`",
        "grep kept < notes.txt",
        "cat 'unclosed",
        "echo done",
        "cd src",
        "",
    ] {
        let refused = project
            .call(BASH, json!({ "command": command }))
            .await
            .expect_err(command);
        assert!(refused.contains(REFUSED), "{command}: {refused}");
    }
    assert_eq!(project.read("notes.txt"), "kept as it was\n");
    assert!(!project.at("copy.txt").exists());
}
