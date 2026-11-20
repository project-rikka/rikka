use rikka_llm_adapter::DynAdapter;

pub(crate) static ALL: &[&dyn DynAdapter] = &[
    &rikka_llm_anthropic::AnthropicAdapter,
    &rikka_llm_openai::OpenAiAdapter,
    &rikka_llm_openrouter::OpenRouterAdapter,
    &rikka_llm_claude_code::ClaudeCodeAdapter,
    &rikka_llm_codex::CodexAdapter,
    &rikka_llm_antigravity::AntigravityAdapter,
];
