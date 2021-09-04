use enum_to_enum::FromEnum;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Src {
    Case1(String),
}

#[derive(Debug, Clone, PartialEq, Eq, FromEnum)]
enum SimpleDest {
    Case1(String),
}

fn main() {}
