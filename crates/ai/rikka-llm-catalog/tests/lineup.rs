use jiff::civil::date;
use rikka_llm::wire::ListedModel;
use rikka_llm::{Effort, ModelInfo, ThinkingOff};
use rikka_llm_catalog::{Catalog, Lookup, lineup};
use serde_json::{Value, json};

fn model(name: &str, released: &str, efforts: &[&str], context: u64) -> Value {
    json!({
        "name": name,
        "tool_call": true,
        "release_date": released,
        "modalities": { "input": ["text", "image"], "output": ["text"] },
        "limit": { "context": context, "output": 128_000 },
        "reasoning_options": [{ "type": "effort", "values": efforts }],
        "cost": { "input": 5.0, "output": 25.0, "cache_read": 0.5 }
    })
}

fn catalog() -> Catalog {
    let document = json!({
        "openai": { "models": {
            "gpt-6.1-terra": model("GPT-6.1 Terra", "2026-10-26", &["none", "low", "max"], 1_050_000),
            "gpt-6.1-sol": model("GPT-6.1 Sol", "2026-09-29", &["low", "high"], 1_050_000),
            "gpt-6-sol": model("GPT-6 Sol", "2026-09-22", &["low", "high"], 1_050_000),
            "gpt-5.4-mini": model("GPT-5.4 mini", "2026-03-17", &["low"], 400_000),
            "gpt-image-2": {
                "name": "GPT Image 2",
                "tool_call": false,
                "release_date": "2026-04-21",
                "modalities": { "input": ["text"], "output": ["image"] },
                "limit": { "context": 0, "output": 0 }
            }
        }},
        "anthropic": { "models": {
            "claude-opus-5-5": model("Claude Opus 5.5", "2026-09-22", &["low", "medium", "high", "xhigh", "max"], 1_000_000),
            "claude-sonnet-4-5": model("Claude Sonnet 4.5 (latest)", "2025-09-29", &[], 200_000)
        }},
        "google": { "models": {
            "gemini-4-hydrogen": model("Gemini 4 Hydrogen", "2026-11-12", &["low", "medium", "high", "xhigh"], 1_048_576),
            "gemini-3.8-flash": model("Gemini 3.8 Flash", "2026-09-02", &["low", "medium", "high"], 1_048_576)
        }}
    });
    Catalog::parse(&serde_json::to_vec(&document).expect("the document should serialize"))
        .expect("the catalog should parse")
}

fn shown(models: &[ModelInfo]) -> (Vec<&str>, Vec<&str>) {
    let current = models
        .iter()
        .filter(|model| !model.is_superseded())
        .map(ModelInfo::id)
        .collect();
    let superseded = models
        .iter()
        .filter(|model| model.is_superseded())
        .map(ModelInfo::id)
        .collect();
    (current, superseded)
}

#[test]
fn bare_listing_is_described_by_the_catalog_and_aged_by_line() {
    let listed = [
        "gpt-6-sol",
        "gpt-6.1-sol",
        "gpt-5.4-mini",
        "gpt-6.1-terra",
        "gpt-image-2",
        "whisper-1",
        "gpt-6.1-sol-2026-09-29",
    ]
    .map(ListedModel::new)
    .to_vec();

    let models = lineup(listed, &catalog(), Lookup::Exact("openai"));

    assert_eq!(
        shown(&models),
        (
            vec!["gpt-6.1-terra", "gpt-6.1-sol"],
            vec!["gpt-6-sol", "gpt-5.4-mini"]
        )
    );
    let terra = &models[0];
    assert_eq!(terra.name(), "GPT-6.1 Terra");
    assert_eq!((terra.context(), terra.output()), (1_050_000, 128_000));
    assert!(terra.vision());
    assert_eq!(terra.efforts(), [Effort::Off, Effort::Low, Effort::Max]);
    let price = terra.price().expect("the catalog prices the model");
    assert_eq!(
        (price.input, price.output, price.cache_read),
        (5.0, 25.0, 0.5)
    );
}

#[test]
fn listed_facts_win_over_the_catalog() {
    let listed = vec![
        ListedModel::new("claude-opus-5-5")
            .with_name(Some("Claude Opus 5.5 (Preview)".to_owned()))
            .with_context(Some(500_000))
            .with_efforts(Some(vec![Effort::High, Effort::Off]))
            .with_thinking_off(Some(ThinkingOff::BetweenTools))
            .with_released(Some(date(2026, 9, 21))),
        ListedModel::new("claude-sonnet-4-5-20250929")
            .with_context(Some(200_000))
            .with_deprecated(true),
        ListedModel::new("claude-sonnet-4-5"),
    ];

    let models = lineup(listed, &catalog(), Lookup::Exact("anthropic"));

    assert_eq!(
        shown(&models),
        (
            vec!["claude-opus-5-5"],
            vec!["claude-sonnet-4-5", "claude-sonnet-4-5-20250929"]
        )
    );
    let opus = &models[0];
    assert_eq!(opus.name(), "Claude Opus 5.5 (Preview)");
    assert_eq!((opus.context(), opus.output()), (500_000, 128_000));
    assert_eq!(opus.efforts(), [Effort::Off, Effort::High]);
    assert_eq!(opus.thinking_off(), ThinkingOff::BetweenTools);
    assert!(opus.price().is_some());
}

#[test]
fn variant_ids_are_described_by_their_longest_catalog_prefix() {
    let listed = vec![
        ListedModel::new("gemini-4-hydrogen-xhigh")
            .with_name(Some("Gemini 4 Hydrogen".to_owned()))
            .with_context(Some(1_048_576))
            .with_output(Some(65_536)),
        ListedModel::new("gemini-3.8-flash-high").with_name(Some("Gemini 3.8 Flash".to_owned())),
        ListedModel::new("gemini-3.8-flash-tiered")
            .with_name(Some("Gemini 3.8 Flash (Tiered)".to_owned())),
        ListedModel::new("claude-opus-5-5-high"),
        ListedModel::new("gpt-oss-120b-medium").with_context(Some(114_000)),
        ListedModel::new("gemini-9-unknown"),
    ];

    let models = lineup(listed, &catalog(), Lookup::Prefix(&["google", "anthropic"]));

    assert_eq!(
        shown(&models),
        (
            vec![
                "gemini-4-hydrogen-xhigh",
                "claude-opus-5-5-high",
                "gemini-3.8-flash-high",
                "gemini-3.8-flash-tiered",
                "gpt-oss-120b-medium"
            ],
            vec![]
        )
    );
    let hydrogen = &models[0];
    assert_eq!(hydrogen.output(), 65_536);
    assert_eq!(
        hydrogen.efforts(),
        [Effort::Low, Effort::Medium, Effort::High, Effort::XHigh]
    );
    assert!(hydrogen.vision());
    let unknown = models
        .iter()
        .find(|model| model.id() == "gpt-oss-120b-medium")
        .expect("a self-described model is kept");
    assert_eq!(
        (unknown.name(), unknown.context(), unknown.output()),
        ("gpt-oss-120b-medium", 114_000, 0)
    );
    assert!(unknown.efforts().is_empty() && !unknown.vision());
}
