// T-012: a signed-out HOME must not produce a saved account or a false login.
#[test]
fn signed_out_capture_is_refused() {
    let home = tempfile::tempdir().unwrap();
    assert!(cc_same_core::cli_accounts::current_login(home.path()).unwrap().is_none());
    assert!(cc_same_core::cli_accounts::add_current(home.path()).is_err());
}
