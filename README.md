# enum_to_enum &emsp; [![CI](https://github.com/ratchetdesigns/enum_to_enum/actions/workflows/ci.yml/badge.svg)](https://github.com/ratchetdesigns/enum_to_enum/actions/workflows/ci.yml) [![Latest Version](https://img.shields.io/crates/v/enum_to_enum.svg)](https://crates.io/crates/enum_to_enum) [![Rust Documentation](https://docs.rs/enum_to_enum/badge.svg)](https://docs.rs/enum_to_enum) ![Crates.io](https://img.shields.io/crates/l/enum_to_enum)

**enum_to_enum exposes a derive macro to easily generate possibly effectful enum-to-enum conversions: `#[derive(FromEnum)]`.**

---

## When should you use enum_to_enum?

Many transformation pipelines are readily expressed as conversions from one enum to another.
However, these transformations can be tedious to write, especially if they generate some additional effects in addition to data mapping.
enum_to_enum makes it easy to generate these conversions.

## enum_to_enum in action

<details>
<summary>
Show cargo.toml
</summary>

```toml
[dependencies]
enum_to_enum = "0.1.0"
```
</details>
<p></p>

```rust
use enum_to_enum::FromEnum;

#[derive(Debug)]
enum Src {
    Case1(),
    Case2(SrcStrField),
    Case3 { a: SrcStrField, b: u8 },
}

#[derive(FromEnum, Debug, PartialEq, Eq)]
#[from_enum(Src)]
enum Dest {
    Case1(),

    #[from_case(Case2)]
    DestCase2(DestStrField),

    #[from_case(Src = Case3)]
    DestCase3 { a: DestStrField, b: u8 },
}

#[derive(Debug, PartialEq, Eq)]
struct SrcStrField(String);

#[derive(Debug, PartialEq, Eq)]
struct DestStrField(String);

impl From<SrcStrField> for DestStrField {
    fn from(src: SrcStrField) -> DestStrField {
        DestStrField(src.0 + " world")
    }
}

assert_eq!(
    Dest::from(Src::Case1()),
    Dest::Case1(),
);

assert_eq!(
    Dest::from(Src::Case2(SrcStrField(String::from("hello")))),
    Dest::DestCase2(DestStrField(String::from("hello world"))),
);

assert_eq!(
    Dest::from(Src::Case3 {
        a: SrcStrField(String::from("hello")),
        b: 100u8,
    }),
    Dest::DestCase3 {
        a: DestStrField(String::from("hello world")),
        b: 100u8,
    },
);
```

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in enum_to_enum by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
</sub>
