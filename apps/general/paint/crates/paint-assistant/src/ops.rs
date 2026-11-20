use std::path::Path;
use std::sync::LazyLock;

use rikka_effects::{ColorLookup, Effect, Kind, Knob, Unit};
use rikka_image_edit::{
    AutoColor, AutoContrast, AutoLevels, Desaturate, Equalize, FindEdges, Invert, Operation,
    Recipe, Sepia, Step,
};
use serde_json::{Map, Value as Json};

use crate::framing::number;
use crate::vocabulary::listed;

pub(crate) type Parsed<T> = Result<T, String>;

const MAX_LOOKUP_BYTES: u64 = 64 << 20;
const TABLE: &str = "table";
const DESATURATIONS: [(&str, Desaturate); 3] = [
    ("lightness", Desaturate::Lightness),
    ("luminosity", Desaturate::Luminosity),
    ("average", Desaturate::Average),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direct {
    AutoLevels,
    AutoContrast,
    AutoColor,
    Equalize,
    Invert,
    Desaturate,
    Sepia,
    FindEdges,
}

impl Direct {
    const ALL: [(&'static str, Direct); 8] = [
        ("auto_levels", Direct::AutoLevels),
        ("auto_contrast", Direct::AutoContrast),
        ("auto_color", Direct::AutoColor),
        ("equalize", Direct::Equalize),
        ("invert", Direct::Invert),
        ("desaturate", Direct::Desaturate),
        ("sepia", Direct::Sepia),
        ("find_edges", Direct::FindEdges),
    ];

    fn operation(self, params: &Map<String, Json>) -> Parsed<Operation> {
        let method = match (self, params.iter().next()) {
            (_, None) => DESATURATIONS[0].1,
            (Direct::Desaturate, Some((name, value))) if name == "method" && params.len() == 1 => {
                let requested = value.as_str().unwrap_or_default();
                DESATURATIONS
                    .iter()
                    .find(|(known, _)| *known == requested)
                    .map(|(_, method)| *method)
                    .ok_or_else(|| {
                        format!(
                            "method is {value}; use {}",
                            listed(DESATURATIONS.iter().map(|(name, _)| *name))
                        )
                    })?
            }
            (Direct::Desaturate, Some((name, _))) => {
                return Err(format!("`desaturate` takes only `method`, not `{name}`"));
            }
            (_, Some((name, _))) => {
                return Err(format!("this op takes no parameters, not `{name}`"));
            }
        };
        Ok(match self {
            Direct::AutoLevels => AutoLevels::new().into(),
            Direct::AutoContrast => AutoContrast::new().into(),
            Direct::AutoColor => AutoColor::new().into(),
            Direct::Equalize => Equalize.into(),
            Direct::Invert => Invert.into(),
            Direct::Desaturate => method.into(),
            Direct::Sepia => Sepia::new().into(),
            Direct::FindEdges => FindEdges.into(),
        })
    }
}

struct Named {
    name: String,
    kind: Kind,
    defaults: Map<String, Json>,
}

static EFFECTS: LazyLock<Vec<Named>> = LazyLock::new(|| {
    Kind::ALL
        .iter()
        .filter_map(|kind| {
            let Ok(Json::Object(mut defaults)) = serde_json::to_value(kind.default_effect()) else {
                return None;
            };
            let Some(Json::String(name)) = defaults.remove("kind") else {
                return None;
            };
            Some(Named {
                name,
                kind: *kind,
                defaults,
            })
        })
        .collect()
});

pub(crate) fn names() -> Vec<&'static str> {
    Direct::ALL
        .iter()
        .map(|(name, _)| *name)
        .chain(EFFECTS.iter().map(|named| named.name.as_str()))
        .collect()
}

pub(crate) fn adjustment(op: &str, params: Map<String, Json>) -> Parsed<Recipe> {
    if let Some((_, direct)) = Direct::ALL.iter().find(|(name, _)| *name == op) {
        return Ok(Recipe::new().then(Step::new(direct.operation(&params)?)));
    }
    let named = EFFECTS
        .iter()
        .find(|named| named.name == op)
        .ok_or_else(|| unknown(op))?;
    let recipe = effect(named, params)?.recipe();
    match recipe.is_empty() {
        true => Err(format!("`{op}` changes nothing with these parameters")),
        false => Ok(recipe),
    }
}

fn unknown(op: &str) -> String {
    let all = names();
    let near: Vec<&str> = all
        .iter()
        .copied()
        .filter(|known| {
            known.contains(op)
                || op.contains(known)
                || op
                    .split('_')
                    .any(|part| part.len() > 3 && known.contains(part))
        })
        .collect();
    match near.is_empty() {
        true => format!("there is no op \"{op}\"; the ops are {}", all.join(", ")),
        false => format!("there is no op \"{op}\"; perhaps {}", near.join(", ")),
    }
}

fn effect(named: &Named, mut params: Map<String, Json>) -> Parsed<Effect> {
    if named.kind == Kind::ColorLookup {
        let path = params.remove(TABLE).ok_or_else(|| {
            format!(
                "`{}` needs `{TABLE}`: the absolute path of a .cube or .3dl file",
                named.name
            )
        })?;
        if let Some(name) = params.keys().next() {
            return Err(format!(
                "`{}` takes only `{TABLE}`, not `{name}`",
                named.name
            ));
        }
        return lookup(&path).map(Effect::from);
    }
    known_fields(&named.defaults, &params, "")?;
    params.insert("kind".to_owned(), Json::String(named.name.clone()));
    let effect: Effect = serde_json::from_value(Json::Object(params))
        .map_err(|error| format!("`{}`: {error}", named.name))?;
    for knob in effect.knobs() {
        let Some(value) = effect.value(knob.key) else {
            continue;
        };
        if !(knob.scale.range.contains(&value)) {
            return Err(format!(
                "`{}` is {}, outside {}",
                knob.key,
                number(f64::from(value)),
                range(knob)
            ));
        }
    }
    Ok(effect.checked())
}

fn known_fields(
    defaults: &Map<String, Json>,
    given: &Map<String, Json>,
    within: &str,
) -> Parsed<()> {
    for (name, value) in given {
        let Some(default) = defaults.get(name) else {
            return Err(format!(
                "`{within}{name}` is no parameter here; the parameters are {}",
                listed(sorted(defaults))
            ));
        };
        if let (Json::Object(defaults), Json::Object(given)) = (default, value) {
            known_fields(defaults, given, &format!("{within}{name}."))?;
        }
    }
    Ok(())
}

fn sorted(fields: &Map<String, Json>) -> Vec<&str> {
    let mut names: Vec<&str> = fields.keys().map(String::as_str).collect();
    names.sort_unstable();
    names
}

fn lookup(value: &Json) -> Parsed<ColorLookup> {
    let path = value
        .as_str()
        .map(Path::new)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| {
            format!("`{TABLE}` takes the absolute path of a .cube or .3dl file, not {value}")
        })?;
    let displayed = path.display();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(extension.as_deref(), Some("cube" | "3dl")) {
        return Err(format!("{displayed} is no .cube or .3dl file"));
    }
    let size = std::fs::metadata(path)
        .map_err(|error| format!("{displayed} cannot be read: {error}"))?
        .len();
    if size > MAX_LOOKUP_BYTES {
        return Err(format!("{displayed} is too large for a lookup table"));
    }
    ColorLookup::read(path).map_err(|error| format!("{displayed}: {error}"))
}

fn unit(unit: Unit) -> &'static str {
    match unit {
        Unit::Plain => "",
        Unit::Pixels => " px",
        Unit::Percent => "%",
        Unit::Degrees => "°",
        Unit::Kelvin => " K",
        Unit::Seconds => " s",
        Unit::Frames => " frames",
    }
}

fn range(knob: &Knob) -> String {
    format!(
        "{}…{}{}",
        number(f64::from(*knob.scale.range.start())),
        number(f64::from(*knob.scale.range.end())),
        unit(knob.scale.unit)
    )
}

pub(crate) fn table() -> String {
    let mut lines = vec![format!(
        "- {}; `desaturate` takes `method` {} =lightness",
        listed(Direct::ALL.iter().map(|(name, _)| *name)),
        DESATURATIONS
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<&str>>()
            .join("|")
    )];
    lines.extend(EFFECTS.iter().map(|named| {
        let fields = match named.kind {
            Kind::ColorLookup => {
                vec![format!(
                    "`{TABLE}` absolute path of a .cube or .3dl file, required"
                )]
            }
            kind => described(kind.default_effect().knobs(), &named.defaults, ""),
        };
        format!("- `{}`: {}", named.name, fields.join("; "))
    }));
    lines.join("\n")
}

fn described(knobs: &[Knob], defaults: &Map<String, Json>, within: &str) -> Vec<String> {
    let mut groups: Vec<(Vec<&str>, String)> = Vec::new();
    let mut fields: Vec<(&String, &Json)> = defaults.iter().collect();
    fields.sort_by_key(|(name, _)| *name);
    for (name, default) in fields {
        let key = format!("{within}{name}");
        let description = match default {
            Json::Object(inner) => format!(
                "{{{}}}",
                described(knobs, inner, &format!("{key}.")).join(", ")
            ),
            Json::Number(value) => match knobs.iter().find(|knob| knob.key == key) {
                Some(knob) => format!(
                    "{} ={}",
                    range(knob),
                    number(value.as_f64().unwrap_or_default())
                ),
                None => format!("={value}"),
            },
            other => format!("={other}"),
        };
        match groups.last_mut() {
            Some((names, last)) if *last == description && default.is_object() => {
                names.push(name);
            }
            _ => groups.push((vec![name], description)),
        }
    }
    groups
        .into_iter()
        .map(|(names, description)| match within.is_empty() {
            true => format!("{} {description}", listed(names)),
            false => format!("{} {description}", names.join("|")),
        })
        .collect()
}
