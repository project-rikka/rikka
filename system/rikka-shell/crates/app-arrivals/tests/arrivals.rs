use app_arrivals::Arrivals;

const FIRST: u64 = 1_000;
const LATER: u64 = 2_000;
const LATEST: u64 = 3_000;

#[test]
fn apps_present_at_first_observation_never_arrive() {
    let mut arrivals = Arrivals::default();
    assert!(arrivals.observe(["firefox.desktop", "gimp.desktop"], FIRST));
    assert!(!arrivals.observe(["firefox.desktop", "gimp.desktop"], LATER));
    assert_eq!(arrivals.arrived("firefox.desktop"), None);
    assert_eq!(arrivals.arrived("gimp.desktop"), None);
}

#[test]
fn installed_app_keeps_its_first_sighting() {
    let mut arrivals = Arrivals::default();
    arrivals.observe(["firefox.desktop"], FIRST);
    assert!(arrivals.observe(["firefox.desktop", "gimp.desktop"], LATER));
    assert!(!arrivals.observe(["firefox.desktop", "gimp.desktop"], LATEST));
    assert_eq!(arrivals.arrived("gimp.desktop"), Some(LATER));
    assert_eq!(arrivals.arrived("firefox.desktop"), None);
}

#[test]
fn reinstalled_app_arrives_again() {
    let mut arrivals = Arrivals::default();
    arrivals.observe(["firefox.desktop", "gimp.desktop"], FIRST);
    assert!(arrivals.observe(["firefox.desktop"], LATER));
    assert_eq!(arrivals.arrived("gimp.desktop"), None);
    arrivals.observe(["firefox.desktop", "gimp.desktop"], LATEST);
    assert_eq!(arrivals.arrived("gimp.desktop"), Some(LATEST));
}

#[test]
fn empty_catalogue_is_ignored() {
    let mut arrivals = Arrivals::default();
    assert!(!arrivals.observe([], FIRST));
    arrivals.observe(["firefox.desktop"], LATER);
    assert_eq!(arrivals.arrived("firefox.desktop"), None);
    assert!(!arrivals.observe([], LATEST));
    arrivals.observe(["firefox.desktop"], LATEST);
    assert_eq!(arrivals.arrived("firefox.desktop"), None);
}

#[test]
fn saved_record_keeps_baseline_and_arrivals() {
    let mut arrivals = Arrivals::default();
    arrivals.observe(["firefox.desktop"], FIRST);
    arrivals.observe(["firefox.desktop", "gimp.desktop"], LATER);
    let saved = serde_json::to_string(&arrivals).expect("arrivals serialize");
    let mut restored: Arrivals = serde_json::from_str(&saved).expect("arrivals deserialize");
    assert!(!restored.observe(["firefox.desktop", "gimp.desktop"], LATEST));
    assert_eq!(restored.arrived("firefox.desktop"), None);
    assert_eq!(restored.arrived("gimp.desktop"), Some(LATER));
}
