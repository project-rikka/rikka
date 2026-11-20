use rikka_nix_protocol::SERVICE;
use rikka_xml::Element;

fn parsed(source: &str) -> Element<'_> {
    rikka_xml::parse(source).expect("policy is valid XML")
}

#[test]
fn apply_action_requires_admin_auth() {
    let policy = parsed(include_str!("../os.rikka.nix.policy"));
    let actions: Vec<_> = policy
        .descendants()
        .filter(|node| node.name() == "action")
        .collect();
    assert_eq!(actions.len(), 1);
    let answer = |tag: &str| {
        actions[0]
            .descendants()
            .find(|node| node.name() == tag)
            .map(|node| node.text().into_owned())
    };
    assert_eq!(answer("allow_any").as_deref(), Some("auth_admin"));
    assert_eq!(answer("allow_inactive").as_deref(), Some("auth_admin"));
    assert_eq!(answer("allow_active").as_deref(), Some("auth_admin_keep"));
}

#[test]
fn only_root_owns_service_name() {
    let conf = parsed(include_str!("../os.rikka.Nix1.conf"));
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

#[test]
fn rebuild_never_stops_service_unit() {
    let mut section = "";
    let mut keys = Vec::new();
    for line in include_str!("../rikka-nixctl.service.in").lines() {
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            section = name;
        } else if let Some((key, value)) = line.split_once('=')
            && key.starts_with("X-")
        {
            keys.push((section, key, value));
        }
    }
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            ("Service", "X-RestartIfChanged", "false"),
            ("Service", "X-StopIfChanged", "false"),
            ("Unit", "X-StopOnRemoval", "false"),
        ]
    );
}
