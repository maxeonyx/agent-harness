use help_test::HelpTest;

#[test]
fn help_examples() {
    HelpTest::new("agent-harness").page(&[], |_| {}).run();
}
