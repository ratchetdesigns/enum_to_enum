use std::convert::From;
use enum_to_enum::{FromEnum, WithEffects};

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
    C2(u16),
    #[from_case(C2)]
    C3(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MyEffect {
    Log(String),
}

#[derive(Debug, Clone, PartialEq, Eq, FromEnum)]
#[from_enum(Src, effect_container = EffectHolder)]
enum EffectDest {
    Case1(String),

    #[from_case(Case2)]
    MyCase2(),

    Case3 { a: String },
}

#[derive(Debug, PartialEq, Eq)]
struct EffectHolder<Value> {
    value: Value,
    effects: Vec<MyEffect>,
}

impl From<String> for EffectHolder<String> {
    fn from(s: String) -> EffectHolder<String> {
        EffectHolder {
            value: s,
            effects: vec![],
        }
    }
}

impl<Value> WithEffects for EffectHolder<Value> {
    type Value = Value;
    type Effect = MyEffect;

    fn new(value: Self::Value, effects: Vec<Self::Effect>) -> Self {
        Self {
            value,
            effects,
        }
    }

    fn effects(&self) -> &[Self::Effect] {
        &self.effects
    }
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
    assert_eq!(
        FallibleDest::from(FallibleSrc::C1(100u16)),
        FallibleDest::C1(100u8)
    );
    assert_eq!(
        FallibleDest::from(FallibleSrc::C1(300u16)),
        FallibleDest::C2(300u16),
    );
    assert_eq!(
        EffectHolder::<EffectDest>::from(Src::Case1("hi".to_string())),
        EffectHolder {
            value: EffectDest::Case1("hi".to_string()),
            effects: vec![],
        },
    );
}
