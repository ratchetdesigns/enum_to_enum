#[test]
fn example_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/examples/*.rs");
}

fn main() {}
