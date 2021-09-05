#![warn(missing_docs)]

//! # enum_to_enum
//!
//! enum_to_enum exposes a derive macro to easily generate possibly effectful enum-to-enum conversions.
//!
//! The destination enum must be annotated to specify one or more source enums we will generate
//! [`From`] implementations for.
//!
//! The generated `From` implementations rely on provided [`From`] or [`TryFrom`](std::convert::TryFrom) implementations for
//! each corresponding field or tuple-item in struct-like or tuple-like enums.
//!
//! Each variant of the destination enum may specify one or more variants of the source enums that
//! should correspond to the destination variant. If multiple variants of a given source enum might
//! correspond to the same destination variant, the destination variant must have at least 1 field
//! or tuple item and the corresponding `TryFrom` implementations will be invoked in the order in
//! which they appear on the destination enum until one of them succeeds.
//!
//! Effectful conversions require users to provide a struct implementing the [`WithEffects`] trait
//! and a conversion will be generated from each source enum to the provided `effect_container`.

#[allow(unused_imports)]
#[macro_use]
extern crate enum_to_enum_derive;

pub use enum_to_enum_derive::*;

/// Any struct specified as an `effect_container` for the [`from_enum`](enum_to_enum_derive::FromEnum) attribute must implement `WithEffects`.
/// `WithEffects` specifies a container for a value, the result of some conversion, and an ordered
/// list of effects arising from that conversion.
///
/// It is expected that these effects correspond to some actions to take subsequent to the
/// conversion.
pub trait WithEffects
where
    Self: Sized,
{
    /// The type of the value we store. This is the result of the conversion.
    type Value;

    /// The type of the effects we store.
    type Effect;

    /// Creates a new instance of this `WithEffects` implementor with the given value and effects.
    fn new(value: Self::Value, effects: Vec<Self::Effect>) -> Self;

    /// Converts self into a tuple of its value and an iterator over its effects.
    fn into_value_and_effects(self) -> (Self::Value, Box<dyn Iterator<Item = Self::Effect>>);

    /// Creates a new instance of this `WithEffects` implementor with the provided value and
    /// effects.
    /// We provide a default implementation in terms of [`new`](Self::new).
    fn compose_from(value: Self::Value, composed_effects: Box<[Self::Effect]>) -> Self {
        let effects = composed_effects.into();
        Self::new(value, effects)
    }
}
