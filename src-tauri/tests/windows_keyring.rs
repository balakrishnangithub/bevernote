#![cfg(target_os = "windows")]

#[test]
fn isolated_credential_roundtrip() {
    let service = format!("app.bevernote.desktop.self-test.{}", uuid::Uuid::new_v4());
    let entry =
        keyring::Entry::new(&service, "pcloud-test").expect("create isolated credential entry");
    entry
        .set_password("isolated-test-token")
        .expect("save isolated test credential");
    let stored = entry.get_password();
    let deleted = entry.delete_credential();
    assert_eq!(
        stored.expect("read isolated test credential"),
        "isolated-test-token"
    );
    deleted.expect("delete isolated test credential");
}
