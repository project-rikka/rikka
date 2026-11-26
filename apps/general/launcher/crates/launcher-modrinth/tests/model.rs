use launcher_modrinth::{DependencyType, ProjectType, SearchResults, Version, VersionType};

#[test]
fn version_parses_api_response() {
    let version: Version = serde_json::from_str(
        r#"{
            "id": "IZskON6d",
            "project_id": "AANobbMI",
            "author_id": "uhPSqlnd",
            "featured": true,
            "name": "Sodium 0.8.0",
            "version_number": "mc26.3-0.8.0",
            "changelog": "Fixes",
            "changelog_url": null,
            "date_published": "2026-09-20T10:00:00.000000Z",
            "downloads": 120,
            "version_type": "release",
            "status": "listed",
            "requested_status": null,
            "files": [
                {"hashes": {"sha512": "aa", "sha1": "bb"}, "url": "https://cdn.modrinth.com/data/AANobbMI/versions/IZskON6d/sodium.jar",
                 "filename": "sodium.jar", "primary": false, "size": 10, "file_type": null},
                {"hashes": {"sha512": "cc", "sha1": "dd"}, "url": "https://cdn.modrinth.com/data/AANobbMI/versions/IZskON6d/sodium-main.jar",
                 "filename": "sodium-main.jar", "primary": true, "size": 20, "file_type": null}
            ],
            "dependencies": [
                {"version_id": null, "project_id": "P7dR8mSH", "file_name": null, "dependency_type": "required"},
                {"version_id": null, "project_id": "YL57xq9U", "file_name": null, "dependency_type": "incompatible"}
            ],
            "game_versions": ["26.3"],
            "loaders": ["fabric", "quilt"]
        }"#,
    )
    .expect("version parses");
    assert_eq!(version.version_type, VersionType::Release);
    assert_eq!(
        version.primary_file().map(|file| file.filename.as_str()),
        Some("sodium-main.jar")
    );
    assert_eq!(
        version.dependencies[1].dependency_type,
        DependencyType::Incompatible
    );
}

#[test]
fn search_tolerates_unknown_project_types() {
    let results: SearchResults = serde_json::from_str(
        r#"{"hits": [
            {"project_id": "a", "slug": "a", "project_type": "minecraft_java_server", "title": "A", "description": "x"},
            {"project_id": "b", "slug": "b", "project_type": "shader", "title": "B", "description": "y", "downloads": 3}
        ], "offset": 0, "limit": 20, "total_hits": 2}"#,
    )
    .expect("results parse");
    assert_eq!(results.hits[0].project_type, ProjectType::Other);
    assert_eq!(results.hits[1].project_type, ProjectType::Shader);
}
