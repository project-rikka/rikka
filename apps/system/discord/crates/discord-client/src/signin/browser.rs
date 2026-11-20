use crate::error::{Error, Result};
use crate::signing::{Prompt, SignInStep, WebSignIn, answered};

const LOGIN_PAGE: &str = "https://discord.com/login";
const MESSAGE: &str = "rikkaDiscordToken";
const SCRIPT: &str = r#"(() => {
  if (window.__rikkaDiscordHooked) return;
  window.__rikkaDiscordHooked = true;
  const post = (token) => {
    if (typeof token !== "string") return;
    const clean = token.trim().replace(/^"|"$/g, "");
    if (clean.length < 20 || clean.includes(" ") || window.__rikkaDiscordSent === clean) return;
    window.__rikkaDiscordSent = clean;
    try { window.rikka.messageHandlers.rikkaDiscordToken.postMessage(clean); } catch (_) {}
  };
  const header = (name, value) => {
    if (typeof name === "string" && name.toLowerCase() === "authorization" && value) post(String(value));
  };
  const answers = /\/api\/v\d+\/auth\/(login|mfa\/[a-z]+|register)(\?|$)/;
  const read = (body) => { if (body && typeof body.token === "string") post(body.token); };
  const setRequestHeader = XMLHttpRequest.prototype.setRequestHeader;
  XMLHttpRequest.prototype.setRequestHeader = function (name, value) {
    header(name, value);
    return setRequestHeader.call(this, name, value);
  };
  const open = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (method, url) {
    if (answers.test(String(url))) {
      this.addEventListener("load", () => {
        try { read(JSON.parse(this.responseText)); } catch (_) {}
      });
    }
    return open.apply(this, arguments);
  };
  const original = window.fetch;
  window.fetch = function (input, init) {
    try {
      const headers = (init && init.headers) || (input instanceof Request ? input.headers : null);
      if (headers instanceof Headers) header("authorization", headers.get("authorization"));
      else if (Array.isArray(headers)) headers.forEach((pair) => header(pair[0], pair[1]));
      else if (headers) Object.keys(headers).forEach((name) => header(name, headers[name]));
    } catch (_) {}
    const url = typeof input === "string" ? input : (input && input.url) || "";
    return original.apply(this, arguments).then((response) => {
      if (answers.test(url)) response.clone().json().then(read).catch(() => {});
      return response;
    });
  };
})();"#;

pub(crate) async fn sign_in(prompt: Prompt) -> Result<String> {
    let (request, answer) = WebSignIn::new(LOGIN_PAGE, SCRIPT, MESSAGE);
    prompt(SignInStep::Web(request));
    let posted = answered(answer).await?;
    let token = posted.trim().trim_matches('"').to_owned();
    if token.is_empty() {
        return Err(Error::Cancelled);
    }
    Ok(token)
}
