use launcher_modrinth::{ProjectType, SearchQuery};

#[test]
fn facets_combine_groups_with_and() {
    let mut query = SearchQuery::new(ProjectType::Mod);
    query.game_version = Some("26.3".to_owned());
    query.loaders = vec!["fabric".to_owned()];
    query.categories = vec!["optimization".to_owned(), "utility".to_owned()];
    let facets = serde_json::to_string(&query.facets()).expect("facets encode");
    assert_eq!(
        facets,
        r#"[["project_type:mod"],["versions:26.3"],["categories:fabric"],["client_side:optional","client_side:required"],["categories:optimization"],["categories:utility"]]"#
    );
}

#[test]
fn packs_skip_client_side_facet() {
    let query = SearchQuery::new(ProjectType::ResourcePack);
    let facets = serde_json::to_string(&query.facets()).expect("facets encode");
    assert_eq!(facets, r#"[["project_type:resourcepack"]]"#);
}
