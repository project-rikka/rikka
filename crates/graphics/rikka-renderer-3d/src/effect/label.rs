use rikka_sync::Mutex;

use rustc_hash::FxHashSet;

static LABELS: Mutex<Option<FxHashSet<&'static str>>> = Mutex::new(None);

pub(crate) fn intern(label: &str) -> &'static str {
    let mut labels = LABELS.lock();
    let labels = labels.get_or_insert_with(FxHashSet::default);
    if let Some(&known) = labels.get(label) {
        return known;
    }
    let leaked: &'static str = Box::leak(label.to_owned().into_boxed_str());
    labels.insert(leaked);
    leaked
}
