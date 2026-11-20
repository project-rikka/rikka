use std::fmt;

use crate::docs::docs;
use crate::hub::Hub;

pub(crate) fn instruction(hub: &Hub) -> String {
    let docs = docs();
    let listed = fmt::from_fn(|out| {
        for standing in hub.statuses() {
            if standing.is_connected() {
                let marked = match standing.offers_resources() {
                    true => " *resources*",
                    false => "",
                };
                match standing.title() {
                    Some(title) => writeln!(out, "- {}: {title}{marked}", standing.server())?,
                    None => writeln!(out, "- {}{marked}", standing.server())?,
                }
            } else if standing.needs_sign_in() {
                writeln!(
                    out,
                    "- {}: waiting for the person to sign in (`sign_in`)",
                    standing.server()
                )?;
            }
        }
        Ok(())
    })
    .to_string();

    let mut facts = docs.facts.clone();
    facts.set(
        "servers",
        match listed.is_empty() {
            true => "No server is connected now.".to_owned(),
            false => format!("Connected now:\n{}", listed.trim_end()),
        },
    );
    docs.servers
        .body(&facts)
        .expect("MCP servers prompt should only reference supplied facts")
}
