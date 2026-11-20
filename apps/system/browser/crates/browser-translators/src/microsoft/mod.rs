mod targets;
mod wire;

use browser_translate::{
    Adapter, Call, Dialect, Error, Http, Limits, Result, Service, Translated, Translating,
};

const ENDPOINT: &str = "https://edge.microsoft.com/translate/translatetext";

static SERVICE: Service = Service {
    id: "microsoft",
    label: "Microsoft",
    dialect: Dialect::new("id").quoted().keep("class=\"notranslate\""),
    targets: targets::TARGETS,
    aliases: &[
        ("sr", "sr-Cyrl"),
        ("mn", "mn-Cyrl"),
        ("ku", "kmr"),
        ("ckb", "ku"),
        ("hmn", "mww"),
        ("lg", "lug"),
        ("mni-Mtei", "mni"),
        ("ny", "nya"),
        ("rn", "run"),
    ],
    limits: Limits::new(100, 5_000),
};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Microsoft;

impl Adapter for Microsoft {
    fn service(&self) -> &'static Service {
        &SERVICE
    }

    fn translate<'a>(&'a self, http: &'a Http, call: Call<'a>) -> Translating<'a> {
        Box::pin(translate(http, call))
    }
}

async fn translate(http: &Http, call: Call<'_>) -> Result<Vec<Translated>> {
    let body = http
        .post(ENDPOINT)
        .query("isEnterpriseClient", "false")
        .query("to", call.to)
        .query("textType", "html")
        .header("content-type", "application/json")
        .body(wire::body(&call))
        .send()
        .await?;
    wire::parse(&body).ok_or(Error::MalformedResponse)
}
