use rikka_prompt::Facts;

pub fn facts_for(language: &str) -> rikka_prompt::Result<Facts> {
    let mut facts = rikka_tool_docs::facts()?;
    facts.set("model", "{{model}}");
    facts.set("language", language);
    facts.set("platform", std::env::consts::OS);
    facts.set("date", jiff::Zoned::now().strftime("%Y-%m-%d").to_string());
    Ok(facts)
}
