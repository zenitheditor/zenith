use std::process::Command;

#[test]
fn library_search_device_finds_lucide_icon_human() {
    let output = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("library")
        .arg("search")
        .arg("device")
        .output()
        .expect("run zenith");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(
        stdout.contains("@zenith/icons-lucide#monitor"),
        "got: {stdout}"
    );
    assert!(stdout.contains("license=ISC"), "got: {stdout}");
    assert!(
        stdout.contains("zenith library add @zenith/icons-lucide#monitor"),
        "got: {stdout}"
    );
}

#[test]
fn library_search_cloud_json_reports_tags() {
    let output = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("library")
        .arg("search")
        .arg("cloud")
        .arg("--json")
        .output()
        .expect("run zenith");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(value["schema"], "zenith-library-search-v1");
    let results = value["results"].as_array().expect("results");
    assert!(
        results
            .iter()
            .any(|result| result["package"] == "@zenith/icons-lucide"
                && result["item"] == "cloud"
                && result["license"] == "ISC"),
        "results: {results:?}"
    );
}
