#![cfg(target_os = "windows")]

use std::process::Command;

#[test]
fn isolated_credential_persists_in_windows_credential_manager() {
    let service = format!("app.bevernote.desktop.self-test.{}", uuid::Uuid::new_v4());
    let target = format!("pcloud-test.{service}");
    let reported_target = format!("LegacyGeneric:target={target}");
    let entry =
        keyring::Entry::new(&service, "pcloud-test").expect("create isolated credential entry");
    entry
        .set_password("isolated-test-token")
        .expect("save isolated test credential");

    // A fresh Entry rules out the in-memory mock's per-entry storage.
    let stored = keyring::Entry::new(&service, "pcloud-test")
        .and_then(|fresh_entry| fresh_entry.get_password());
    let listed = Command::new("cmdkey").arg("/list").output().map(|output| {
        output.status.success()
            && String::from_utf8_lossy(&output.stdout).lines().any(|line| {
                line.split_once(':').is_some_and(|(label, value)| {
                    label.trim().eq_ignore_ascii_case("Target") && value.trim() == reported_target
                })
            })
    });

    // Only the unique test credential is removed; do this before any assertions.
    let deleted = entry.delete_credential();

    assert_eq!(
        stored.expect("read isolated credential through a fresh entry"),
        "isolated-test-token"
    );
    assert!(
        listed.expect("query Windows Credential Manager"),
        "the isolated credential must be visible to Windows Credential Manager"
    );
    deleted.expect("delete isolated test credential");
}
