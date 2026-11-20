use rikka_llm::{Error, Model, Response, Stop};
use rikka_llm_providers::{Endpoint, EndpointCredentials, Header, Listing, Offered, Wire};
use rikka_test_llm::{Exchange, Reply, Server};
use serde_json::{Value, json};

const KEY: &str = "notarealkey";
const MODEL: &str = "qwen3:8b";
const CHAT: &str = "/v1/chat/completions";
const MODELS: &str = "/v1/models";

fn endpoint(server: &Server, wire: Wire) -> Endpoint {
    Endpoint::new(server.url())
        .expect("the server's address should be an endpoint")
        .with_wire(wire)
}

fn credentials() -> EndpointCredentials {
    let header = Header::new("x-gateway-team", "rikka").expect("header should be valid");
    EndpointCredentials::new(KEY, vec![header])
}

fn connected(server: &Server, wire: Wire) -> Model {
    endpoint(server, wire)
        .connect(&credentials(), MODEL)
        .expect("model should connect")
}

fn streaming(frames: &[&str]) -> Server {
    let reply = Reply::events(frames);
    Server::routed(move |exchange: &Exchange| match exchange.path() {
        CHAT => reply.clone(),
        _ => Reply::json(404, &json!({ "error": "not found" })),
    })
}

async fn answer(model: &Model) -> rikka_llm::Result<Response> {
    model
        .chat()
        .user("hi")
        .max_tokens(256)
        .stream()
        .await?
        .response()
        .await
}

fn asked(server: &Server, path: &str) -> Exchange {
    server
        .exchanges()
        .into_iter()
        .find(|exchange| exchange.path() == path)
        .unwrap_or_else(|| panic!("nothing asked {path}"))
}

#[test]
fn base_address_is_normalized() {
    for (typed, base) in [
        ("http://localhost:11434", "http://localhost:11434/v1"),
        ("  http://127.0.0.1:1234/v1/ ", "http://127.0.0.1:1234/v1"),
        (
            "https://gateway.example/openai/v1/chat/completions?api-version=1",
            "https://gateway.example/openai/v1",
        ),
        (
            "http://192.168.1.20:8000/v1/models",
            "http://192.168.1.20:8000/v1",
        ),
    ] {
        let endpoint = Endpoint::new(typed).expect("address should be accepted");
        assert_eq!(endpoint.base(), base, "{typed}");
    }
    for refused in [
        "localhost:11434",
        "ftp://example.test/v1",
        "http://someone:secret@example.test/v1",
        "",
    ] {
        assert!(
            matches!(Endpoint::new(refused), Err(Error::InvalidEndpoint(_))),
            "{refused}"
        );
    }
}

#[tokio::test]
async fn streamed_answer_reaches_the_endpoint() {
    let server = streaming(&[
        r#"{"id":"chatcmpl-1","model":"qwen3:8b","choices":[{"index":0,"delta":{"role":"assistant","content":"Hel"}}]}"#,
        r#"{"id":"chatcmpl-1","choices":[{"index":0,"delta":{"content":"lo"},"finish_reason":"stop"}]}"#,
        r#"{"id":"chatcmpl-1","choices":[],"usage":{"prompt_tokens":30,"completion_tokens":5}}"#,
        "[DONE]",
    ]);
    let response = answer(&connected(&server, Wire::Completions))
        .await
        .expect("answer should stream");

    assert_eq!(response.text(), "Hello");
    assert_eq!(response.stop(), &Stop::End);
    assert_eq!(response.usage().output, 5);
    let asked = asked(&server, CHAT);
    assert_eq!(asked.method(), "POST");
    assert_eq!(asked.header("authorization"), Some("Bearer notarealkey"));
    assert_eq!(asked.header("x-gateway-team"), Some("rikka"));
    let body = asked.body().expect("a body should be sent");
    assert_eq!(body["model"], json!(MODEL));
    assert_eq!(body["max_tokens"], json!(256));
    for absent in ["max_completion_tokens", "prompt_cache_key"] {
        assert!(body.get(absent).is_none(), "{absent} was sent");
    }
}

#[tokio::test]
async fn keyless_endpoint_sends_no_authorization() {
    let server = streaming(&[r#"{"choices":[{"delta":{"content":"ok"}}]}"#, "[DONE]"]);
    let model = endpoint(&server, Wire::Completions)
        .connect(&EndpointCredentials::default(), MODEL)
        .expect("model should connect");
    answer(&model).await.expect("answer should stream");

    assert_eq!(asked(&server, CHAT).header("authorization"), None);
}

#[tokio::test]
async fn tool_call_deltas_assemble() {
    let cases: [(&str, &[&str]); 4] = [
        (
            "split arguments",
            &[
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","type":"function","function":{"name":"read","arguments":""}}]}}]}"#,
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"path\":"}}]}}]}"#,
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"a.txt\"}"}}]}}]}"#,
                r#"{"choices":[{"delta":{"tool_calls":[{"index":1,"id":"call_b","function":{"name":"list","arguments":"{\"path\":\"b\"}"}}]},"finish_reason":"tool_calls"}]}"#,
                "[DONE]",
            ],
        ),
        (
            "whole calls stopped as an ordinary answer",
            &[
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","function":{"name":"read","arguments":"{\"path\":\"a.txt\"}"}},{"index":1,"id":"call_b","function":{"name":"list","arguments":"{\"path\":\"b\"}"}}]}}]}"#,
                r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
                "[DONE]",
            ],
        ),
        (
            "unnumbered calls without ids",
            &[
                r#"{"choices":[{"delta":{"tool_calls":[{"function":{"name":"read","arguments":"{\"path\":\"a.txt\"}"}}]}}]}"#,
                r#"{"choices":[{"delta":{"tool_calls":[{"function":{"name":"list","arguments":"{\"path\":\"b\"}"}}]},"finish_reason":"tool_calls"}]}"#,
                "[DONE]",
            ],
        ),
        (
            "arguments as objects under one index",
            &[
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","function":{"name":"read","arguments":{"path":"a.txt"}}}]}}]}"#,
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_b","function":{"name":"list","arguments":{"path":"b"}}}]},"finish_reason":"tool_calls"}]}"#,
                "[DONE]",
            ],
        ),
    ];

    for (case, frames) in cases {
        let server = streaming(frames);
        let response = answer(&connected(&server, Wire::Completions))
            .await
            .unwrap_or_else(|error| panic!("{case}: {error}"));

        assert_eq!(response.stop(), &Stop::ToolUse, "{case}");
        let calls: Vec<(&str, &Value)> = response
            .tool_calls()
            .map(|call| (call.name(), call.input()))
            .collect();
        assert_eq!(
            calls,
            [
                ("read", &json!({ "path": "a.txt" })),
                ("list", &json!({ "path": "b" }))
            ],
            "{case}"
        );
        let ids: Vec<&str> = response.tool_calls().map(|call| call.id()).collect();
        assert!(
            ids.iter().all(|id| !id.is_empty()) && ids[0] != ids[1],
            "{case}: {ids:?}"
        );
    }
}

#[tokio::test]
async fn inline_thoughts_become_reasoning() {
    let server = streaming(&[
        r#"{"choices":[{"delta":{"content":"<thi"}}]}"#,
        r#"{"choices":[{"delta":{"content":"nk>weighing it</th"}}]}"#,
        r#"{"choices":[{"delta":{"content":"ink>\n\nIt is 4."},"finish_reason":"stop"}]}"#,
        "[DONE]",
    ]);
    let response = answer(&connected(&server, Wire::Completions))
        .await
        .expect("answer should stream");

    assert_eq!(response.text(), "It is 4.");
    let thoughts: Vec<&str> = response
        .reasoning()
        .map(|reasoning| reasoning.text())
        .collect();
    assert_eq!(thoughts, ["weighing it"]);
}

#[tokio::test]
async fn refusals_are_read_from_each_shape() {
    let cases = [
        (
            400,
            json!({ "object": "error", "message": "This model's maximum context length is 4096 tokens. However, you requested 9000 tokens.", "type": "BadRequestError", "code": 400 }),
            "context overflow",
        ),
        (
            400,
            json!({ "error": { "code": 400, "message": "the request exceeds the available context size, try increasing it", "type": "exceed_context_size_error" } }),
            "context overflow",
        ),
        (
            404,
            json!({ "error": { "message": "model \"qwen3:8b\" not found, try pulling it first", "type": "api_error" } }),
            "unknown model",
        ),
        (
            400,
            json!({ "error": "Invalid model identifier" }),
            "rejected: Invalid model identifier",
        ),
        (
            401,
            json!({ "error": { "message": "invalid key", "code": "invalid_api_key" } }),
            "unauthorized",
        ),
    ];

    for (status, body, expected) in cases {
        let refusal = body.clone();
        let server = Server::routed(move |_: &Exchange| Reply::json(status, &refusal));
        let error = answer(&connected(&server, Wire::Completions))
            .await
            .expect_err("the server refused");
        let read = match &error {
            Error::ContextOverflow => "context overflow".to_owned(),
            Error::UnknownModel(_) => "unknown model".to_owned(),
            Error::Unauthorized => "unauthorized".to_owned(),
            Error::Rejected(reason) => format!("rejected: {reason}"),
            other => format!("{other:?}"),
        };
        assert_eq!(read, expected, "{body}");
    }
}

#[tokio::test]
async fn streamed_error_ends_the_answer() {
    for frame in [
        r#"{"error":{"message":"the backend crashed"}}"#,
        r#"{"error":"the backend crashed"}"#,
        r#"{"object":"error","message":"the backend crashed","type":"InternalServerError","code":500}"#,
    ] {
        let server = streaming(&[r#"{"choices":[{"delta":{"content":"partial"}}]}"#, frame]);
        let error = answer(&connected(&server, Wire::Completions))
            .await
            .expect_err("the stream failed");

        assert!(
            matches!(&error, Error::Rejected(reason) if reason == "the backend crashed"),
            "{frame}: {error:?}"
        );
    }
}

#[tokio::test]
async fn responses_wire_streams_without_hosted_fields() {
    let server = Server::routed(|exchange: &Exchange| match exchange.path() {
        "/v1/responses" => Reply::events(&[
            r#"{"type":"response.created","response":{"id":"resp_1","model":"qwen3:8b"}}"#,
            r#"{"type":"response.output_text.delta","delta":"Hello"}"#,
            r#"{"type":"response.completed","response":{"id":"resp_1","model":"qwen3:8b","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hello"}]}],"usage":{"input_tokens":9,"output_tokens":2}}}"#,
        ]),
        _ => Reply::json(404, &json!({ "error": "not found" })),
    });
    let response = answer(&connected(&server, Wire::Responses))
        .await
        .expect("answer should stream");

    assert_eq!(response.text(), "Hello");
    let asked = asked(&server, "/v1/responses");
    assert_eq!(asked.header("authorization"), Some("Bearer notarealkey"));
    let body = asked.body().expect("a body should be sent");
    assert_eq!(body["max_output_tokens"], json!(256));
    for absent in ["include", "prompt_cache_key"] {
        assert!(body.get(absent).is_none(), "{absent} was sent");
    }
}

type Read<'a> = (
    &'a str,
    Option<u64>,
    Option<bool>,
    Option<bool>,
    Option<bool>,
);

#[tokio::test]
async fn models_are_read_from_each_server() {
    let cases: [(&str, Value, Read<'_>); 6] = [
        (
            "ollama",
            json!({ "object": "list", "data": [{ "id": "qwen3:8b", "object": "model", "created": 1, "owned_by": "library" }] }),
            ("qwen3:8b", None, None, None, None),
        ),
        (
            "llama.cpp",
            json!({ "models": [{ "name": "qwen3-8b.gguf", "model": "qwen3-8b.gguf" }], "data": [{ "id": "qwen3-8b.gguf", "object": "model", "meta": { "n_ctx_train": 40960 } }] }),
            ("qwen3-8b.gguf", Some(40960), None, None, None),
        ),
        (
            "vllm",
            json!({ "object": "list", "data": [{ "id": "Qwen/Qwen3-8B", "object": "model", "max_model_len": 32768 }] }),
            ("Qwen/Qwen3-8B", Some(32768), None, None, None),
        ),
        (
            "lm studio",
            json!({ "data": [
                { "id": "text-embedding-nomic", "type": "embeddings", "max_context_length": 2048 },
                { "id": "qwen2.5-vl-7b", "type": "vlm", "max_context_length": 128000, "capabilities": ["tool_use"] },
            ] }),
            (
                "qwen2.5-vl-7b",
                Some(128000),
                Some(true),
                Some(true),
                Some(false),
            ),
        ),
        (
            "gateway",
            json!({ "data": [{ "id": "deepseek/deepseek-r1", "name": "DeepSeek R1", "context_length": 163840,
                "architecture": { "input_modalities": ["text"] },
                "top_provider": { "max_completion_tokens": 32768 },
                "supported_parameters": ["tools", "reasoning"] }] }),
            (
                "deepseek/deepseek-r1",
                Some(163840),
                Some(false),
                Some(true),
                Some(true),
            ),
        ),
        (
            "mistral",
            json!({ "object": "list", "data": [{ "id": "mistral-small-latest", "max_context_length": 131072,
                "capabilities": { "completion_chat": true, "function_calling": true, "vision": true } }] }),
            (
                "mistral-small-latest",
                Some(131072),
                Some(true),
                Some(true),
                None,
            ),
        ),
    ];

    for (server_kind, listed, expected) in cases {
        let server = Server::routed(move |exchange: &Exchange| match exchange.path() {
            MODELS => Reply::json(200, &listed),
            _ => Reply::json(404, &json!({})),
        });
        let listing = endpoint(&server, Wire::Completions)
            .models(&credentials())
            .await
            .unwrap_or_else(|error| panic!("{server_kind}: {error}"));
        let models = listing.models();

        assert_eq!(models.len(), 1, "{server_kind}: {models:?}");
        let model = &models[0];
        let read = (
            model.id(),
            model.context(),
            model.vision(),
            model.tools(),
            model.reasoning(),
        );
        assert_eq!(read, expected, "{server_kind}");
        assert_eq!(
            asked(&server, MODELS).header("authorization"),
            Some("Bearer notarealkey")
        );
    }
}

#[test]
fn unreported_capabilities_get_defaults() {
    let unlisted = Offered::unlisted(" llama3.2:3b ").model_info();

    assert_eq!(unlisted.id(), "llama3.2:3b");
    assert!(unlisted.context() > 0 && unlisted.output() > 0);
    assert!(u64::from(unlisted.output()) <= unlisted.context());
    assert!(unlisted.tools() && !unlisted.vision() && unlisted.efforts().is_empty());
}

#[tokio::test]
async fn server_without_models_is_checked_by_answering() {
    let server = Server::routed(|exchange: &Exchange| match exchange.path() {
        CHAT => Reply::events(&[r#"{"choices":[{"delta":{"content":"OK"}}]}"#, "[DONE]"]),
        _ => Reply::text(404, "404 page not found"),
    });
    let endpoint = endpoint(&server, Wire::Completions);

    assert_eq!(
        endpoint
            .models(&credentials())
            .await
            .expect("a missing list is no failure"),
        Listing::Unlisted
    );
    let checkup = endpoint
        .check(&credentials(), Some(MODEL))
        .await
        .expect("the endpoint should answer");
    assert_eq!(checkup.listing(), &Listing::Unlisted);
    assert_eq!(checkup.answered(), Some(MODEL));

    let unreachable = Endpoint::new("http://127.0.0.1:9/v1")
        .expect("address should be accepted")
        .check(&credentials(), None)
        .await;
    assert!(matches!(unreachable, Err(Error::Connect(_))));
}

#[tokio::test]
async fn ollama_models_are_described_by_the_server() {
    let server = Server::routed(|exchange: &Exchange| match exchange.path() {
        MODELS => Reply::json(
            200,
            &json!({ "object": "list", "data": [
                { "id": "qwen3:8b", "object": "model", "owned_by": "library" },
                { "id": "deepseek-r1:14b", "object": "model", "owned_by": "library" },
            ] }),
        ),
        "/api/version" => Reply::json(200, &json!({ "version": "0.13.0" })),
        "/api/show" => match exchange
            .body()
            .and_then(|body| body["model"].as_str().map(str::to_owned))
            .as_deref()
        {
            Some("qwen3:8b") => Reply::json(
                200,
                &json!({
                    "parameters": "num_ctx                        16384\nstop                           \"<|im_end|>\"",
                    "model_info": { "general.architecture": "qwen3", "qwen3.context_length": 40960 },
                    "capabilities": ["completion", "tools", "vision"],
                }),
            ),
            Some("deepseek-r1:14b") => Reply::json(
                200,
                &json!({
                    "model_info": { "general.architecture": "qwen2", "qwen2.context_length": 131072 },
                    "capabilities": ["completion", "thinking"],
                }),
            ),
            _ => Reply::json(404, &json!({ "error": "model not found" })),
        },
        _ => Reply::json(404, &json!({})),
    });

    let listing = endpoint(&server, Wire::Completions)
        .models(&credentials())
        .await
        .expect("the models should be listed");
    let read: Vec<Read<'_>> = listing
        .models()
        .iter()
        .map(|model| {
            (
                model.id(),
                model.context(),
                model.vision(),
                model.tools(),
                model.reasoning(),
            )
        })
        .collect();

    assert_eq!(
        read,
        [
            ("qwen3:8b", Some(16384), Some(true), Some(true), Some(false)),
            (
                "deepseek-r1:14b",
                Some(131072),
                Some(false),
                Some(false),
                Some(true)
            ),
        ]
    );
    assert_eq!(
        asked(&server, "/api/show").header("authorization"),
        Some("Bearer notarealkey")
    );
}
