use super::*;

#[test]
fn empty_list_and_unknown_inspect_are_explicit() {
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let delegator = Delegator::new(root.path(), "/does/not/run");
    let listed = delegator
        .list()
        .unwrap_or_else(|error| panic!("list: {error}"));
    assert!(listed.tasks.is_empty());
    let error = delegator
        .inspect(&TaskHandle {
            id: "tsk_343434343434343434343434".to_owned(),
        })
        .err()
        .unwrap_or_else(|| panic!("inspect should reject unknown task"));
    assert!(matches!(error, DelegationError::UnknownHandle(_)));
}
