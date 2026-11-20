use rikka_llm::{Effort, Request, Reveal, Ttl};

pub(crate) fn declared(request: &Request, model: &str) -> String {
    let mut betas = vec!["claude-code-20250219", "oauth-2025-04-20"];

    if model.contains("[1m]") {
        betas.push("context-1m-2025-08-07");
    }
    betas.push("interleaved-thinking-2025-05-14");
    if !displays_thinking(request) {
        betas.push("redact-thinking-2026-02-12");
    }
    betas.extend([
        "thinking-token-count-2026-05-13",
        "context-management-2025-06-27",
        "prompt-caching-scope-2026-01-05",
    ]);
    if !request.tools().is_empty() {
        betas.push("advanced-tool-use-2025-11-20");
    }
    betas.push("effort-2025-11-24");
    if request.format().is_some() {
        betas.push("structured-outputs-2025-12-15");
    }
    betas.push("fallback-credit-2026-06-01");
    if request.ttl() == Ttl::Long {
        betas.push("extended-cache-ttl-2025-04-11");
    }

    betas.join(",")
}

fn displays_thinking(request: &Request) -> bool {
    match (request.effort(), request.reveal()) {
        (Some(Effort::Off), _) => false,
        (Some(_), _) | (None, Reveal::Summary) => true,
        (None, Reveal::Hidden) => false,
    }
}
