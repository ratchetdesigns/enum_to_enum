use std::convert::From;
use from_enum::FromEnum;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Src {
    Case1(String),
    Case2(),
}

mod inner {
    pub enum Src {
        C1(String),
        C2(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, FromEnum)]
#[from_enum(Src)]
enum SimpleDest {
    Case1(String),

    #[from_case(Case2)]
    MyCase2(),
}

#[derive(Debug, Clone, PartialEq, Eq, FromEnum)]
#[from_enum(Src, inner::Src)]
enum CompoundDest {
    #[from_case(inner::Src = C1, Src = Case1)]
    Case1(String),

    #[from_case(Src = Case2, inner::Src = C2)]
    MyCase2(),
}

fn main() {
    assert_eq!(
        SimpleDest::from(Src::Case1("hi".to_string())),
        SimpleDest::Case1("hi".to_string())
    );
    assert_eq!(
        CompoundDest::from(inner::Src::C1("a".to_string())),
        CompoundDest::Case1("a".to_string())
    );
}
