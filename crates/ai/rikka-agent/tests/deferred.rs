use rikka_agent::{
    Agent, Deferred, Function, Reply, Tool, ToolKit, ToolKitRegistry, ToolKitSet, Value, json,
};
use rikka_test_llm::{Asked, Scripted};

fn weather() -> Tool {
    Tool::new("get_weather")
        .description("Get current weather")
        .string("location", "City name")
        .build()
        .expect("tool should be valid")
}

fn search() -> Tool {
    Tool::new("find_tool")
        .description("Reveal a tool by name")
        .string("name", "What it is called")
        .build()
        .expect("tool should be valid")
}

async fn answer_weather(_input: Value) -> Result<String, String> {
    Ok("28C".to_owned())
}

fn named(input: &Value) -> Result<String, String> {
    input
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "no name".to_owned())
}

fn declared(request: &Asked) -> Vec<(String, bool)> {
    request.body()["tools"]
        .as_array()
        .expect("tools should be declared")
        .iter()
        .map(|tool| {
            let name = tool["name"].as_str().unwrap_or_default().to_owned();
            (name, tool["defer_loading"] == json!(true))
        })
        .collect()
}

fn pairs(listed: &[(&str, bool)]) -> Vec<(String, bool)> {
    listed
        .iter()
        .map(|(name, deferred)| ((*name).to_owned(), *deferred))
        .collect()
}

fn agent(script: &Scripted, deferred: &Deferred) -> Agent {
    let revealing = deferred.clone();

    Agent::new(script.model())
        .tool(weather(), answer_weather)
        .tool(search(), move |input: Value| {
            let revealing = revealing.clone();
            async move {
                let name = named(&input)?;
                revealing.reveal(&name);
                Ok::<_, String>(format!("{name} is declared from here on"))
            }
        })
        .deferred(deferred.clone())
        .build()
        .expect("agent configuration should be valid")
}

#[tokio::test]
async fn revealed_tool_is_no_longer_deferred() {
    let script = Scripted::new();
    script
        .asks("find_tool", json!({ "name": "get_weather" }))
        .asks("get_weather", json!({ "location": "Tokyo" }))
        .says("It is 28C in Tokyo.");

    let deferred = Deferred::new();
    deferred.defer("get_weather");

    let outcome = agent(&script, &deferred)
        .run("weather in Tokyo?")
        .await
        .expect("run should finish");

    assert_eq!(outcome.text(), "It is 28C in Tokyo.");

    let requests = script.requests();
    assert_eq!(
        declared(&requests[0]),
        pairs(&[("get_weather", true), ("find_tool", false)])
    );
    assert_eq!(
        declared(&requests[1]),
        pairs(&[("get_weather", false), ("find_tool", false)])
    );
}

#[tokio::test]
async fn loaded_tool_keeps_declarations_stable() {
    let script = Scripted::new();
    script
        .asks("find_tool", json!({ "name": "get_weather" }))
        .asks("get_weather", json!({ "location": "Tokyo" }))
        .says("It is 28C in Tokyo.");

    let deferred = Deferred::new();
    deferred.defer("get_weather");

    let outcome = Agent::new(script.model())
        .tool(weather(), answer_weather)
        .tool(search(), |input: Value| async move {
            let name = named(&input)?;
            Ok::<_, String>(Reply::new(format!("{name} is loaded")).load_tool(name))
        })
        .deferred(deferred)
        .build()
        .expect("agent configuration should be valid")
        .run("weather in Tokyo?")
        .await
        .expect("run should finish");

    assert_eq!(outcome.text(), "It is 28C in Tokyo.");

    let requests = script.requests();
    assert_eq!(requests.len(), 3);
    let first = requests[0].body()["tools"].clone();
    for request in &requests[1..] {
        assert_eq!(request.body()["tools"], first);
    }
    assert_eq!(
        requests[1].body()["messages"][2]["content"][0]["content"],
        json!([{ "type": "tool_reference", "tool_name": "get_weather" }])
    );
}

fn posing() -> Tool {
    Tool::new("Animate_Pose")
        .description("Keys a pose")
        .string("figure", "Whom to pose")
        .build()
        .expect("tool should be valid")
}

async fn answer_pose(_input: Value) -> Result<String, String> {
    Ok("posed".to_owned())
}

fn with_toolkits(script: &Scripted, deferred: &Deferred, registry: &ToolKitRegistry) -> Agent {
    let revealing = deferred.clone();

    Agent::new(script.model())
        .tool(search(), move |input: Value| {
            let revealing = revealing.clone();
            async move {
                let name = named(&input)?;
                revealing.reveal(&name);
                Ok::<_, String>(format!("{name} is declared from here on"))
            }
        })
        .deferred(deferred.clone())
        .toolkits(registry.clone())
        .build()
        .expect("agent configuration should be valid")
}

#[tokio::test]
async fn registered_tool_runs_after_reveal() {
    let script = Scripted::new();
    script
        .asks("find_tool", json!({ "name": "Animate_Pose" }))
        .asks("Animate_Pose", json!({ "figure": "Miku" }))
        .says("Posed.");

    let apps = ToolKitSet::new();
    let servers = ToolKitSet::new();
    let registry = ToolKitRegistry::new([apps, servers.clone()]);
    servers
        .replace([ToolKit::new("Animate").with_functions([Function::new(posing(), answer_pose)])]);
    let deferred = Deferred::new();

    let outcome = with_toolkits(&script, &deferred, &registry)
        .run("pose her")
        .await
        .expect("run should finish");

    assert_eq!(outcome.text(), "Posed.");
    let requests = script.requests();
    assert_eq!(
        declared(&requests[0]),
        pairs(&[("find_tool", false), ("Animate_Pose", true)])
    );
    assert_eq!(
        declared(&requests[1]),
        pairs(&[("find_tool", false), ("Animate_Pose", false)])
    );
    assert_eq!(requests.len(), 3);
}
