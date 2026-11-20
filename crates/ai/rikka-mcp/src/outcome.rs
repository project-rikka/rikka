use rikka_agent::Reply;

use crate::part::Part;
use crate::protocol::CallToolResult;
use crate::said::Said;

pub(crate) fn replied(result: CallToolResult) -> Result<Reply, String> {
    let failed = result.is_error.unwrap_or(false);

    let mut said = Said::default();
    for block in result.content {
        said.add(Part::from_block(block));
    }
    if said.is_empty()
        && let Some(structured) = result.structured_content
    {
        said.add(Part::Text(structured.to_string()));
    }

    if failed {
        let text = said.text();
        return Err(match text.is_empty() {
            true => String::from("the server said the call went wrong, and no more"),
            false => text,
        });
    }

    if said.is_empty() {
        return Ok(Reply::new("the call came to nothing at all"));
    }
    Ok(said.into_reply())
}
