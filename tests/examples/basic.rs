use std::convert::From;
use from_enum::FromEnum;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Src {
    Case1(String),
    Case2(),
    Case3 { a: String },
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

    Case3 { a: String },
}

#[derive(Debug, Clone, PartialEq, Eq, FromEnum)]
#[from_enum(Src, inner::Src)]
enum CompoundDest {
    #[from_case(inner::Src = C1, Src = Case1)]
    Case1(String),

    #[from_case(Src = Case2, inner::Src = C2)]
    MyCase2(),

    #[from_case(Src = Case3)]
    Case3 { a: String }
}

enum FallibleSrc {
    C1(u16),
    C2(u16),
}

#[derive(Debug, Clone, PartialEq, Eq, FromEnum)]
#[from_enum(FallibleSrc)]
enum FallibleDest {
    #[from_case(C1)]
    C1(u8),
    #[from_case(C1, C2)]
    C2(u8),
    #[from_case(C2)]
    C3(u8),
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
