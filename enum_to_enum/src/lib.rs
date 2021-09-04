#[allow(unused_imports)]
#[macro_use]
extern crate enum_to_enum_derive;

pub use enum_to_enum_derive::*;

pub trait WithEffects
where
    Self: Sized,
{
    type Value;
    type Effect;

    fn new(value: Self::Value, effects: Vec<Self::Effect>) -> Self;

    fn effects(&self) -> &[Self::Effect];

    fn compose_from(value: Self::Value, composed_effects: Box<[Self::Effect]>) -> Self {
        let effects = composed_effects.into();
        Self::new(value, effects)
    }
}
