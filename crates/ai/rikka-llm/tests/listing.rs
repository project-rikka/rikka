use rikka_llm::wire::listing::{anthropic, cloud_code, codex, openai};
use rikka_llm::wire::{ListedModel, Transport, endpoint, headers};
use rikka_llm::{Effort, Error, ServerTool, ThinkingOff};
use rikka_test_llm::{Exchange, Reply, Server};
use serde_json::{Value, json};

fn serving(answer: impl Fn(&Exchange) -> Value + Send + Sync + 'static) -> (Server, Transport) {
    let server = Server::routed(move |exchange| Reply::json(200, &answer(exchange)));
    let transport = Transport::new(
        endpoint(&format!("{}/v1/models", server.url())).expect("test server URL should parse"),
        headers().build().expect("no headers should build"),
    )
    .expect("test transport should build");
    (server, transport)
}

fn released(model: &ListedModel) -> Option<String> {
    model.released().map(|day| day.to_string())
}

#[tokio::test]
async fn anthropic_listing_follows_pages_and_reads_capabilities() {
    let (server, transport) = serving(|exchange| match exchange.path().contains("after_id=") {
        false => json!({
            "data": [{
                "id": "claude-sonnet-5-5",
                "display_name": "Claude Sonnet 5.5",
                "created_at": "2026-09-28T00:00:00Z",
                "lifecycle": "active",
                "max_input_tokens": 1_000_000,
                "max_tokens": 128_000,
                "capabilities": {
                    "image_input": { "supported": true },
                    "effort": {
                        "supported": true,
                        "low": { "supported": true },
                        "high": { "supported": true },
                        "max": { "supported": false }
                    },
                    "thinking": {
                        "supported": true,
                        "types": {
                            "adaptive": { "supported": true },
                            "between_tools": { "supported": true }
                        }
                    },
                    "server_tools": {
                        "supported": true,
                        "web_search": { "supported": true },
                        "code_execution": { "supported": true }
                    }
                }
            }],
            "has_more": true,
            "last_id": "claude-sonnet-5-5"
        }),
        true => json!({
            "data": [
                {
                    "id": "claude-sonnet-4-5-20250929",
                    "display_name": "Claude Sonnet 4.5",
                    "lifecycle": "deprecated",
                    "capabilities": null
                },
                { "id": "claude-2", "lifecycle": "retired" }
            ],
            "has_more": false,
            "last_id": "claude-2"
        }),
    });

    let listed = anthropic::models(&transport)
        .await
        .expect("listing should be read");

    let paths: Vec<String> = server
        .exchanges()
        .iter()
        .map(|exchange| exchange.path().to_owned())
        .collect();
    assert_eq!(
        paths,
        [
            "/v1/models?limit=1000",
            "/v1/models?limit=1000&after_id=claude-sonnet-5-5"
        ]
    );
    assert_eq!(listed.len(), 2);
    let current = &listed[0];
    assert_eq!(current.name(), Some("Claude Sonnet 5.5"));
    assert_eq!(
        (current.context(), current.output(), current.vision()),
        (Some(1_000_000), Some(128_000), Some(true))
    );
    assert_eq!(
        current.efforts(),
        Some([Effort::Off, Effort::Low, Effort::High].as_slice())
    );
    assert_eq!(current.thinking_off(), Some(ThinkingOff::BetweenTools));
    assert_eq!(
        current.server_tools(),
        Some([ServerTool::WebSearch, ServerTool::WebFetch].as_slice())
    );
    assert_eq!(released(current).as_deref(), Some("2026-09-28"));
    assert!(!current.is_deprecated());
    assert!(listed[1].is_deprecated());
    assert_eq!(listed[1].efforts(), None);
    assert_eq!(listed[1].server_tools(), None);
}

#[tokio::test]
async fn openai_listing_reads_release_and_shutdown() {
    let (_server, transport) = serving(|_| {
        json!({
            "object": "list",
            "data": [
                { "id": "gpt-6.1-sol", "created": 1_790_640_000, "owned_by": "openai", "shutdown_date": null },
                { "id": "gpt-5.5", "created": 1_776_902_400, "owned_by": "openai", "shutdown_date": "2026-12-01" },
                { "id": " ", "created": 1_776_902_400 }
            ]
        })
    });

    let listed = openai::models(&transport)
        .await
        .expect("listing should be read");

    let ids: Vec<&str> = listed.iter().map(ListedModel::id).collect();
    assert_eq!(ids, ["gpt-6.1-sol", "gpt-5.5"]);
    assert_eq!(released(&listed[0]).as_deref(), Some("2026-09-29"));
    assert!(!listed[0].is_deprecated() && listed[1].is_deprecated());
    assert_eq!(
        (listed[0].name(), listed[0].context(), listed[0].efforts()),
        (None, None, None)
    );
}

#[tokio::test]
async fn codex_listing_keeps_listed_models_in_priority_order() {
    let (_server, transport) = serving(|_| {
        json!({
            "models": [
                {
                    "slug": "gpt-5.5",
                    "display_name": "GPT-5.5",
                    "visibility": "list",
                    "priority": 13,
                    "context_window": 272_000,
                    "supported_reasoning_levels": [{ "effort": "low", "description": "" }],
                    "upgrade": { "model": "gpt-6.1-sol", "migration_markdown": "" }
                },
                {
                    "slug": "gpt-6.1-sol",
                    "display_name": "GPT-6.1-Sol",
                    "visibility": "list",
                    "priority": 1,
                    "context_window": 272_000,
                    "input_modalities": ["text", "image"],
                    "supported_reasoning_levels": [
                        { "effort": "xhigh", "description": "" },
                        { "effort": "low", "description": "" },
                        { "effort": "ultra", "description": "" }
                    ]
                },
                { "slug": "codex-auto-review", "visibility": "hide", "priority": 43 }
            ]
        })
    });

    let listed = codex::models(&transport)
        .await
        .expect("listing should be read");

    let ids: Vec<&str> = listed.iter().map(ListedModel::id).collect();
    assert_eq!(ids, ["gpt-6.1-sol", "gpt-5.5"]);
    assert_eq!(listed[0].name(), Some("GPT-6.1-Sol"));
    assert_eq!(listed[0].context(), Some(272_000));
    assert_eq!(listed[0].vision(), Some(true));
    assert_eq!(
        listed[0].efforts(),
        Some([Effort::Low, Effort::XHigh].as_slice())
    );
    assert!(listed[1].is_deprecated());
}

#[tokio::test]
async fn cloud_code_listing_folds_reasoning_variants_of_the_agent_lineup() {
    let variant = |name: &str, level: u64| {
        json!({
            "displayName": name,
            "supportsImages": true,
            "supportsThinking": true,
            "thinkingLevel": level,
            "maxTokens": 1_048_576,
            "maxOutputTokens": 262_144
        })
    };
    let (server, transport) = serving(move |_| {
        json!({
            "models": {
                "gemini-4-hydrogen-low": variant("Gemini 4 Hydrogen (Low)", 1),
                "gemini-4-hydrogen-xhigh": variant("Gemini 4 Hydrogen (XHigh)", 100),
                "claude-opus-5-5-high": variant("Claude Opus 5.5 (High)", 3),
                "claude-opus-5-5-low": variant("Claude Opus 5.5 (Low)", 1),
                "gpt-oss-120b-medium": {
                    "displayName": "GPT-OSS 120B (Medium)",
                    "supportsThinking": true,
                    "thinkingBudget": 8192,
                    "maxTokens": 131_072,
                    "maxOutputTokens": 32_768
                },
                "gemini-3-flash": variant("Gemini 3 Flash", 3),
                "tab_flash_lite_preview": { "maxTokens": 16_384 }
            },
            "agentModelSorts": [{
                "displayName": "Recommended",
                "groups": [{ "modelIds": [
                    "gemini-4-hydrogen-xhigh",
                    "gemini-4-hydrogen-low",
                    "claude-opus-5-5-low",
                    "claude-opus-5-5-high",
                    "gpt-oss-120b-medium",
                    "gemini-9-gone-high"
                ]}]
            }],
            "commandModelIds": ["gemini-3-flash"]
        })
    });

    let listed = cloud_code::models(&transport, "projects/42")
        .await
        .expect("listing should be read");

    assert_eq!(server.last(), json!({ "project": "projects/42" }));
    let read: Vec<_> = listed
        .iter()
        .map(|model| (model.id(), model.name(), model.efforts()))
        .collect();
    assert_eq!(
        read,
        [
            (
                "gemini-4-hydrogen",
                Some("Gemini 4 Hydrogen"),
                Some([Effort::Low, Effort::XHigh].as_slice())
            ),
            (
                "claude-opus-5-5",
                Some("Claude Opus 5.5"),
                Some([Effort::Low, Effort::High].as_slice())
            ),
            (
                "gpt-oss-120b-medium",
                Some("GPT-OSS 120B (Medium)"),
                Some([].as_slice())
            ),
        ]
    );
    assert_eq!(
        (listed[0].context(), listed[0].output(), listed[0].vision()),
        (Some(1_048_576), Some(262_144), Some(true))
    );
}

#[tokio::test]
async fn cloud_code_listing_without_agent_lineup_keeps_named_public_models() {
    let (_server, transport) = serving(|_| {
        json!({
            "models": {
                "gemini-3-flash": { "displayName": "Gemini 3 Flash" },
                "chat_20706": { "displayName": "Chat", "isInternal": true },
                "tab_flash_lite_preview": {}
            }
        })
    });

    let listed = cloud_code::models(&transport, "projects/42")
        .await
        .expect("listing should be read");

    let ids: Vec<&str> = listed.iter().map(ListedModel::id).collect();
    assert_eq!(ids, ["gemini-3-flash"]);
}

#[tokio::test]
async fn listing_without_its_models_is_malformed() {
    let (_server, transport) = serving(|_| json!({ "object": "list" }));

    for listed in [
        anthropic::models(&transport).await,
        openai::models(&transport).await,
        codex::models(&transport).await,
        cloud_code::models(&transport, "projects/42").await,
    ] {
        assert!(matches!(listed, Err(Error::Malformed(_))), "{listed:?}");
    }
}
