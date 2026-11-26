use rikka_nix::{AttrPath, Document, Expr, Set};
use rikka_settings::Value;

use crate::survey::Setting;

const USERS: [&str; 2] = ["rikka", "users"];

const SETTINGS: &str = "settings";

pub fn path(user: &str, setting: &Setting) -> rikka_nix::Result<AttrPath> {
    let mut segments: Vec<&str> = USERS.to_vec();
    segments.extend([user, SETTINGS, setting.id.domain.as_str()]);
    segments.extend(setting.instance.as_deref());
    segments.push(&setting.key);
    Ok(AttrPath::new(segments)?)
}

#[must_use]
pub fn expr(value: &Value) -> Expr {
    match value {
        Value::Bool(value) => Expr::Bool(*value),
        Value::Int(value) => Expr::Int(*value),
        Value::Float(value) => Expr::Float(*value),
        Value::String(value) => Expr::String(value.clone()),
        Value::List(items) => Expr::List(items.iter().map(expr).collect()),
        Value::Table(values) => Expr::Set(values.iter().fold(Set::new(), |set, (name, value)| {
            set.bind(vec![name.clone()], expr(value))
        })),
    }
}

#[must_use]
pub fn declares(document: &Document, user: &str, setting: &Setting) -> Option<Expr> {
    let path = path(user, setting).ok()?;
    document.get::<Expr>(&path).ok().flatten()
}
