use rikka_app::prelude::*;

const WIDTH: f32 = 220.0;

pub(crate) fn picker<M>(
    selected: Option<usize>,
    options: impl IntoIterator<Item = Label>,
    choose: impl IntoHandler<(usize,), (), M>,
) -> impl View {
    combo(selected)
        .options(options)
        .on_select(choose)
        .width(WIDTH)
}

pub(crate) fn choose<T: Copy + PartialEq + 'static, M>(
    options: &'static [(T, &'static str)],
    selected: T,
    on_choose: impl IntoHandler<(T,), (), M>,
) -> impl View {
    picker(
        options.iter().position(|(value, _)| *value == selected),
        options.iter().map(|(_, key)| Label::translated(key)),
        on_choose.into_handler().map(move |(index,): (usize,)| {
            (options.get(index).map_or(selected, |(value, _)| *value),)
        }),
    )
}

pub(crate) fn labelled(options: &[(&str, &'static str)]) -> Vec<(String, Label)> {
    options
        .iter()
        .map(|(value, key)| ((*value).to_owned(), Label::translated(key)))
        .collect()
}

pub(crate) fn value_picker<M>(
    unset: Label,
    options: Vec<(String, Label)>,
    current: Option<&str>,
    choose: impl IntoHandler<(Option<String>,), (), M>,
) -> impl View {
    let mut options = options;
    if let Some(current) = current
        && !options.iter().any(|(value, _)| value == current)
    {
        options.insert(0, (current.to_owned(), Label::from(current)));
    }
    let selected =
        current.and_then(|current| options.iter().position(|(value, _)| value == current));
    let (values, labels): (Vec<String>, Vec<Label>) = options.into_iter().unzip();
    default_picker(
        unset,
        labels,
        selected,
        choose
            .into_handler()
            .map(move |(index,): (Option<usize>,)| {
                (index.and_then(|index| values.get(index).cloned()),)
            }),
    )
}

pub(crate) fn default_picker<M>(
    default: Label,
    options: impl IntoIterator<Item = Label>,
    selected: Option<usize>,
    choose: impl IntoHandler<(Option<usize>,), (), M>,
) -> impl View {
    picker(
        Some(selected.map_or(0, |index| index + 1)),
        std::iter::once(default).chain(options),
        choose
            .into_handler()
            .map(|(index,): (usize,)| (index.checked_sub(1),)),
    )
}
