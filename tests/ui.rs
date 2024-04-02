#[test]
fn test_ui() {
    trycmd::TestCases::new()
        .default_bin_name("quake")
        .case("tests/ui/*.toml");
}
