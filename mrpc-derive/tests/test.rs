#[test]
fn test() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/null_server.rs");
}
