use crate::abi::{self, Function, Lifecycle, Origin, USERDATA};
use crate::languages::c::spelling::Spelling;
use crate::model::{
    Callback, DeclarationKind, Input, Lifetime, Member, Output, Ownership, Param, Threading,
};
use crate::names::spaced;

pub(super) fn callback(spelling: &Spelling, callback: &Callback) -> Vec<String> {
    let mut docs = spelling.body(&callback.doc.body);
    for param in &callback.params {
        let text = spelling.text(&param.doc);
        if !text.is_empty() {
            docs.push(format!("@param {} {text}", param.name));
        }
    }
    docs.push(format!(
        "@param {USERDATA} The userdata given with the closure."
    ));
    docs
}

pub(super) fn function(spelling: &Spelling, function: &Function) -> Vec<String> {
    let api = spelling.api;
    let mut docs = spelling.body(function.description(api));
    if let Origin::Member(class, index) = function.origin
        && let Member::Declaration(declaration) = api.member(class, index)
        && declaration.kind == DeclarationKind::Container
    {
        docs.push(format!(
            "End it with {}().",
            spelling.function(Origin::End(class))
        ));
    }
    match function.origin {
        Origin::End(_) => docs.push(
            "Ends the innermost begun element, which becomes the current element.".to_owned(),
        ),
        Origin::Lifecycle(class, lifecycle) => {
            let name = spaced(&api[class].name);
            let article = match name.starts_with(['a', 'e', 'i', 'o', 'u']) {
                true => "an",
                false => "a",
            };
            docs.push(match lifecycle {
                Lifecycle::Free => format!(
                    "Destroys {article} {name} that was never handed over. Passing NULL does nothing."
                ),
                Lifecycle::Ref => {
                    format!("Adds a reference to {article} {name}. Passing NULL returns NULL.")
                }
                Lifecycle::Unref => format!(
                    "Drops a reference to {article} {name}; the last one destroys it. Passing NULL does nothing."
                ),
            });
        }
        Origin::Function(_) | Origin::Member(..) | Origin::Setter(..) => {}
    }
    if function.thread_safe {
        docs.push("Thread-safe.".to_owned());
    }
    if let Some(receiver) = function.receiver
        && receiver.ownership == Ownership::Owned
        && !matches!(function.origin, Origin::Lifecycle(..))
    {
        docs.push(format!(
            "@param {} Ownership passes to the SDK.",
            abi::receiver_name(&api[receiver.class])
        ));
    }
    for param in &function.signature.params {
        docs.extend(param_docs(spelling, param));
    }
    let returned = [
        returns_doc(spelling, function),
        returns_note(spelling, function),
    ]
    .into_iter()
    .filter(|text| !text.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    if !returned.is_empty() {
        docs.push(format!("@return {returned}"));
    }
    docs
}

fn returns_doc(spelling: &Spelling, function: &Function) -> String {
    match (function.origin, function.doc(spelling.api)) {
        (Origin::Lifecycle(class, Lifecycle::Ref), _) => {
            format!("`{}` itself.", abi::receiver_name(&spelling.api[class]))
        }
        (_, Some(doc)) => spelling.text(&doc.returns),
        (_, None) => String::new(),
    }
}

fn returns_note(spelling: &Spelling, function: &Function) -> String {
    let api = spelling.api;
    if let Some(error) = function.signature.throws {
        let codes = api[error]
            .error
            .expect("validated: thrown enums are errors");
        return format!(
            "{}, or the failure; {}() describes it.",
            spelling.variant(error, codes.success),
            spelling.function(Origin::Function(codes.message))
        );
    }
    if matches!(function.origin, Origin::Lifecycle(..)) {
        return String::new();
    }
    match function.signature.returns {
        Some(Output::String(Ownership::Owned)) => format!(
            "The caller owns the string; release it with {}_string_free().",
            spelling.prefix()
        ),
        Some(Output::Object {
            class,
            ownership: Ownership::Owned,
        }) => {
            let release = match api[class].lifetime {
                Lifetime::Owned => Lifecycle::Free,
                _ => Lifecycle::Unref,
            };
            format!(
                "The caller owns the result; release it with {}().",
                spelling.function(Origin::Lifecycle(class, release))
            )
        }
        Some(Output::Object {
            class,
            ownership: Ownership::Borrowed,
        }) => format!(
            "Borrowed; call {}() to keep it.",
            spelling.function(Origin::Lifecycle(class, Lifecycle::Ref))
        ),
        _ => String::new(),
    }
}

fn param_docs(spelling: &Spelling, param: &Param) -> Vec<String> {
    let doc = spelling.text(&param.doc);
    let note = match &param.ty {
        Input::Object {
            ownership: Ownership::Owned,
            ..
        } => "Ownership passes to the SDK.".to_owned(),
        Input::Array(_) => {
            let count = abi::count_name(&param.name);
            format!("Points to `{count}` elements; may be NULL when `{count}` is 0.")
        }
        Input::Closure(closure) => {
            let mut docs = Vec::new();
            for function in &closure.functions {
                let single = closure.functions.len() == 1;
                let text = spelling
                    .body(abi::closure_description(
                        spelling.api,
                        param,
                        function,
                        single,
                    ))
                    .join(" ");
                docs.push(format!("@param {} {text}", function.name));
            }
            let names = closure
                .functions
                .iter()
                .map(|function| format!("`{}`", function.name))
                .collect::<Vec<_>>()
                .join(" and ");
            docs.push(format!("@param {USERDATA} Passed to {names}."));
            let thread = match closure.threading {
                Threading::AnyThread => " It may run on any thread.",
                Threading::ThisThread => "",
            };
            docs.push(format!(
                "@param {} Called with `{USERDATA}` once the SDK no longer needs it; may be NULL.{thread}",
                abi::DESTROY
            ));
            return docs;
        }
        Input::Value(_) | Input::String | Input::Object { .. } => String::new(),
    };
    let text = [doc, note]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    match text.is_empty() {
        true => Vec::new(),
        false => vec![format!("@param {} {text}", param.name)],
    }
}
