use rikka_boost::service::SERVICE;

#[test]
fn only_root_owns_service_name() {
    let conf =
        rikka_xml::parse(include_str!("../os.rikka.Boost1.conf")).expect("bus policy is valid XML");
    let owners: Vec<(Option<&str>, &str)> = conf
        .children_named("policy")
        .flat_map(|policy| {
            policy
                .children_named("allow")
                .filter_map(|allow| Some((policy.attribute("user"), allow.attribute("own")?)))
        })
        .collect();
    assert_eq!(owners, [(Some("root"), SERVICE)]);
}
