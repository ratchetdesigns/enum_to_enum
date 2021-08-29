use std::convert::From;
use from_enum::FromEnum;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Src {
    Case1(),
    Case2(),
}

mod inner {
    pub enum Src {
        C1(),
        C2(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, FromEnum)]
#[from_enum(Src)]
enum SimpleDest {
    Case1(),

    #[from_case(OtherCase2)]
    MyCase2(),
}

#[derive(Debug, Clone, PartialEq, Eq, FromEnum)]
#[from_enum(Src, inner::Src)]
enum CompoundDest {
    Case1(),

    #[from_case(Src = OtherCase2, Src = OtherCase1)]
    MyCase2(),
}

fn main() {
    assert_eq!(SimpleDest::from(Src::Case1()), SimpleDest::Case1());
    assert_eq!(CompoundDest::from(inner::Src::C1()), CompoundDest::Case1());
}
