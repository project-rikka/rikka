use rikka_agent::{Function, Reply};
use rikka_llm::{Tool, Value, json};
use rikka_tools::TOOL_SEARCH;

use crate::support::Project;

fn deferred_tool(name: &str, description: &str) -> Function {
    let tool = Tool::new(name)
        .description(description)
        .string("url", "The page to fetch")
        .build()
        .expect("tool should be valid");
    Function::new(tool, |_: Value| async {
        Ok::<_, String>("done".to_owned())
    })
}

fn project() -> Project {
    Project::fitted(|setup| {
        setup.deferred([
            deferred_tool(
                "WebFetch",
                "Fetches a page from the web and reads it as Markdown",
            ),
            deferred_tool(
                "WebSearch",
                "Searches the web and hands back what was found",
            ),
            deferred_tool("Notebook", "Edits one cell of a Jupyter notebook"),
        ])
    })
}

async fn search(project: &Project, input: Value) -> Reply {
    project
        .call(TOOL_SEARCH, input)
        .await
        .expect("search should succeed")
}

#[tokio::test]
async fn named_tool_loads_by_reference() {
    let project = project();
    let deferred = project.toolbox().deferred();

    let found = search(&project, json!({ "query": "select:webfetch, Nowhere" })).await;

    assert_eq!(found.loaded_tools(), ["WebFetch"]);
    assert!(found.text().contains("WebFetch"), "{}", found.text());
    assert!(deferred.is_hidden("WebFetch"));
    assert!(deferred.is_hidden("WebSearch"));
}

#[tokio::test]
async fn keyword_search_ranks_closest_first() {
    let project = project();

    let found = search(&project, json!({ "query": "notebook jupyter" })).await;
    assert_eq!(found.loaded_tools(), ["Notebook"]);

    let found = search(&project, json!({ "query": "web page" })).await;
    assert_eq!(found.loaded_tools(), ["WebFetch", "WebSearch"]);

    let found = search(&project, json!({ "query": "+notebook web" })).await;
    assert_eq!(found.loaded_tools(), ["Notebook"]);

    let found = search(&project, json!({ "query": "web page", "max_results": 1 })).await;
    assert_eq!(found.loaded_tools(), ["WebFetch"]);

    assert!(
        project
            .call(TOOL_SEARCH, json!({ "query": "" }))
            .await
            .is_err()
    );
}
