use enum_to_enum::FromEnum;

enum Src {
    Case1(String),
}

#[derive(FromEnum)]
#[from_enum(Src, effect_container = BadEffectHolder)]
enum EffectDest {
    Case1(String),
}

struct BadEffectHolder<V> {
    v: V,
}

fn main() {}
