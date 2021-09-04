#[allow(unused_imports)]
#[macro_use]
extern crate enum_to_enum_derive;

pub use enum_to_enum_derive::*;

#[derive(Debug, Clone)]
pub struct WithEffects<Value, Effect> {
    pub value: Value,
    pub effects: Vec<Effect>,
}
