#[test]
fn example_passing_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/examples/*_pass.rs");
}

#[test]
fn example_failing_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/examples/*_fail.rs");
}

fn main() {}
