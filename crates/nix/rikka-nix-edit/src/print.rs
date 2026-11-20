use crate::expr::Expr;
use crate::lambda::{Formals, Param};
use crate::name;
use crate::set::{Entry, Set};

const WIDTH: usize = 100;

const STEP: usize = 2;

const SIMPLE_FORMALS: usize = 2;

#[derive(Clone, Copy)]
enum Place {
    Item,
    Function,
    Scope,
}

pub(crate) fn document(header: Option<&str>, expr: &Expr) -> String {
    let mut out = String::new();
    if let Some(header) = header {
        for line in header.lines() {
            comment(&mut out, line);
            out.push('\n');
        }
    }
    root(&mut out, expr);
    out.push('\n');
    out
}

pub(crate) fn binding_into(out: &mut String, key: &str, value: &Expr, indent: usize) {
    out.push_str(key);
    out.push_str(" =");
    match value {
        Expr::Lambda {
            param: Param::Formals(formals),
            body,
        } => {
            let deeper = indent + STEP;
            newline(out, deeper);
            self::formals(out, formals, deeper);
            out.push(':');
            newline(out, deeper);
            write(out, body, deeper, 1);
        }
        Expr::Let { .. } => {
            let deeper = indent + STEP;
            newline(out, deeper);
            write(out, value, deeper, 1);
        }
        _ => {
            out.push(' ');
            write(out, value, indent, 1);
        }
    }
    out.push(';');
}

pub(crate) fn entry_into(out: &mut String, entry: &Entry, indent: usize) {
    match entry {
        Entry::Binding { path, value } => binding_into(out, &path.to_string(), value, indent),
        Entry::Dynamic { key, value } => {
            let mut interpolated = String::from("${");
            write(&mut interpolated, key, indent, 0);
            interpolated.push('}');
            binding_into(out, &interpolated, value, indent);
        }
        Entry::Inherit { from, names } => {
            out.push_str("inherit");
            if let Some(from) = from {
                out.push_str(" (");
                write(out, from, indent, 1);
                out.push(')');
            }
            for name in names {
                out.push(' ');
                out.push_str(name);
            }
            out.push(';');
        }
        Entry::Comment(text) => {
            for (index, line) in lines(text).enumerate() {
                if index > 0 {
                    newline(out, indent);
                }
                comment(out, line);
            }
        }
        Entry::Blank => {}
    }
}

fn root(out: &mut String, expr: &Expr) {
    match expr {
        Expr::Lambda {
            param: Param::Formals(formals),
            body,
        } => {
            self::formals(out, formals, 0);
            out.push_str(":\n");
            root(out, body);
        }
        _ => write(out, expr, 0, 0),
    }
}

fn write(out: &mut String, expr: &Expr, indent: usize, tail: usize) {
    if let Some(text) = flat(expr)
        && (column(out) + width(&text) + tail <= WIDTH || !breaks(expr))
    {
        out.push_str(&text);
        return;
    }
    match expr {
        Expr::List(items) => {
            out.push('[');
            for item in items {
                newline(out, indent + STEP);
                operand(out, item, Place::Item, indent + STEP, 0);
            }
            newline(out, indent);
            out.push(']');
        }
        Expr::Set(set) => self::set(out, set, indent),
        Expr::Apply { function, argument } => {
            operand(out, function, Place::Function, indent, 0);
            out.push(' ');
            operand(out, argument, Place::Item, indent, tail);
        }
        Expr::With { scope, body } => {
            out.push_str("with ");
            operand(out, scope, Place::Scope, indent, 1);
            out.push_str("; ");
            write(out, body, indent, tail);
        }
        Expr::Let { bindings, body } => {
            out.push_str("let");
            entries(out, bindings, indent);
            newline(out, indent);
            out.push_str("in");
            newline(out, indent);
            write(out, body, indent, tail);
        }
        Expr::Lambda {
            param: Param::Name(name),
            body,
        } => {
            out.push_str(name);
            out.push_str(": ");
            write(out, body, indent, tail);
        }
        Expr::Lambda {
            param: Param::Formals(formals),
            body,
        } => {
            self::formals(out, formals, indent);
            out.push(':');
            newline(out, indent);
            write(out, body, indent, tail);
        }
        Expr::Null
        | Expr::Bool(_)
        | Expr::Int(_)
        | Expr::Float(_)
        | Expr::String(_)
        | Expr::Path(_)
        | Expr::Reference(_) => out.push_str(&atom(expr)),
    }
}

fn operand(out: &mut String, expr: &Expr, place: Place, indent: usize, tail: usize) {
    match parenthesized(expr, place) {
        true => {
            out.push('(');
            write(out, expr, indent, tail + 1);
            out.push(')');
        }
        false => write(out, expr, indent, tail),
    }
}

fn parenthesized(expr: &Expr, place: Place) -> bool {
    match place {
        Place::Item => {
            matches!(
                expr,
                Expr::Apply { .. } | Expr::Lambda { .. } | Expr::With { .. } | Expr::Let { .. }
            ) || negative(expr)
        }
        Place::Function => {
            matches!(
                expr,
                Expr::Lambda { .. } | Expr::With { .. } | Expr::Let { .. }
            ) || negative(expr)
        }
        Place::Scope => matches!(expr, Expr::Lambda { .. } | Expr::Let { .. }),
    }
}

fn negative(expr: &Expr) -> bool {
    match expr {
        Expr::Int(value) => *value < 0,
        Expr::Float(value) => value.is_sign_negative(),
        _ => false,
    }
}

fn set(out: &mut String, set: &Set, indent: usize) {
    out.push('{');
    entries(out, set, indent);
    newline(out, indent);
    out.push('}');
}

fn entries(out: &mut String, set: &Set, indent: usize) {
    for entry in set.entries() {
        match entry {
            Entry::Blank => out.push('\n'),
            entry => {
                newline(out, indent + STEP);
                entry_into(out, entry, indent + STEP);
            }
        }
    }
}

pub(crate) fn formals(out: &mut String, formals: &Formals, indent: usize) {
    let mut items: Vec<String> = formals
        .formals()
        .iter()
        .map(|formal| match &formal.default {
            Some(default) => {
                let mut item = format!("{} ? ", formal.name);
                write(&mut item, default, indent + STEP, 1);
                item
            }
            None => formal.name.clone(),
        })
        .collect();
    if formals.is_open() {
        items.push("...".to_owned());
    }
    if let Some(bound) = formals.bound_name() {
        out.push_str(bound);
        out.push('@');
    }
    let together = match items.is_empty() {
        true => "{ }".to_owned(),
        false => format!("{{ {} }}", items.join(", ")),
    };
    let simple = formals.formals().len() <= SIMPLE_FORMALS
        && formals
            .formals()
            .iter()
            .all(|formal| formal.default.is_none());
    if simple && !together.contains('\n') && column(out) + width(&together) < WIDTH {
        out.push_str(&together);
        return;
    }
    out.push('{');
    for item in &items {
        newline(out, indent + STEP);
        out.push_str(item);
        if item != "..." {
            out.push(',');
        }
    }
    newline(out, indent);
    out.push('}');
}

fn flat(expr: &Expr) -> Option<String> {
    match expr {
        Expr::List(items) => match items.as_slice() {
            [] => Some("[ ]".to_owned()),
            [only] => {
                let inner = flat(only)?;
                Some(match parenthesized(only, Place::Item) {
                    true => format!("[ ({inner}) ]"),
                    false => format!("[ {inner} ]"),
                })
            }
            _ => None,
        },
        Expr::Set(set) => set.entries().is_empty().then(|| "{ }".to_owned()),
        Expr::Apply { function, argument } => {
            let function = wrapped(function, Place::Function)?;
            let argument = wrapped(argument, Place::Item)?;
            Some(format!("{function} {argument}"))
        }
        Expr::With { scope, body } => {
            let scope = wrapped(scope, Place::Scope)?;
            Some(format!("with {scope}; {}", flat(body)?))
        }
        Expr::Lambda {
            param: Param::Name(name),
            body,
        } => Some(format!("{name}: {}", flat(body)?)),
        Expr::Lambda {
            param: Param::Formals(_),
            ..
        }
        | Expr::Let { .. } => None,
        Expr::Null
        | Expr::Bool(_)
        | Expr::Int(_)
        | Expr::Float(_)
        | Expr::String(_)
        | Expr::Path(_)
        | Expr::Reference(_) => Some(atom(expr)),
    }
}

fn wrapped(expr: &Expr, place: Place) -> Option<String> {
    let text = flat(expr)?;
    Some(match parenthesized(expr, place) {
        true => format!("({text})"),
        false => text,
    })
}

fn breaks(expr: &Expr) -> bool {
    match expr {
        Expr::List(items) => !items.is_empty(),
        Expr::Set(set) => !set.entries().is_empty(),
        Expr::Apply { .. } | Expr::With { .. } | Expr::Lambda { .. } | Expr::Let { .. } => true,
        _ => false,
    }
}

fn atom(expr: &Expr) -> String {
    match expr {
        Expr::Null => "null".to_owned(),
        Expr::Bool(value) => value.to_string(),
        Expr::Int(value) => value.to_string(),
        Expr::Float(value) => float(*value),
        Expr::String(text) => name::quote(text),
        Expr::Path(text) => text.clone(),
        Expr::Reference(path) => path.to_string(),
        _ => String::new(),
    }
}
fn float(value: f64) -> String {
    let text = format!("{value:?}");
    if text.contains('.') {
        return text;
    }
    match text.split_once('e') {
        Some((mantissa, exponent)) => format!("{mantissa}.0e{exponent}"),
        None => format!("{text}.0"),
    }
}

fn comment(out: &mut String, line: &str) {
    match line.trim_end() {
        "" => out.push('#'),
        line => {
            out.push_str("# ");
            out.push_str(line);
        }
    }
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        lines.push("");
    }
    lines.into_iter()
}

fn newline(out: &mut String, indent: usize) {
    out.push('\n');
    out.extend(std::iter::repeat_n(' ', indent));
}

fn column(out: &str) -> usize {
    out.rsplit('\n').next().map_or(0, width)
}

fn width(text: &str) -> usize {
    text.chars().count()
}
