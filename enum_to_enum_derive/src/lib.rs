extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::convert::From;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::fs::File;
use syn::{
    parenthesized,
    parse::{Error as ParseError, Parse, ParseStream, Result as ParseResult},
    parse2,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Comma as CommaToken, Eq as EqToken},
    visit::{visit_item_enum, Visit},
    Attribute, Error as SynError, Fields, Ident, ItemEnum, Path, Token, Type, Variant,
};

#[proc_macro_derive(FromEnum, attributes(from_enum, from_case))]
pub fn derive_enum_from(input: TokenStream) -> TokenStream {
    let result = from_enum_internal(input.into()).unwrap_or_else(|err| {
        let errors = err.into_compile_errors();
        quote! {
            #(#errors)*
        }
    });

    let mut file = File::create(name).expect("failed to create file");
    std::io::Write::write_all(&mut file, result.to_string().as_bytes()).expect("failed to write");

    result.into()
}

#[derive(Debug, Clone)]
struct ConversionCfg {
    src_case: SrcCase,
    dest: Variant,
}

impl ConversionCfg {
    fn each_arg<F: Fn(&Ident, &Type) -> TokenStream2>(&self, xform: F) -> Vec<TokenStream2> {
        match &self.dest.fields {
            Fields::Unit => vec![],
            Fields::Named(named) => named
                .named
                .iter()
                .map(|field| xform(field.ident.as_ref().unwrap(), &field.ty))
                .collect(),
            Fields::Unnamed(unnamed) => unnamed
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, field)| xform(&format_ident!("arg{}", i), &field.ty))
                .collect(),
        }
    }

    fn to_args<T: Fn(&Ident, &Type) -> TokenStream2>(&self, xform: T) -> TokenStream2 {
        let args = self.each_arg(xform);
        quote! {
            #(#args),*
        }
    }

    fn to_wrapped_args<T: Fn(&Ident) -> TokenStream2>(&self, xform: T) -> TokenStream2 {
        let args = self.to_args(|id, _| xform(id));

        match &self.dest.fields {
            Fields::Unit => quote! {},
            Fields::Named(_) => {
                quote! {
                    { #args }
                }
            }
            Fields::Unnamed(_) => {
                quote! {
                    (#args)
                }
            }
        }
    }

    fn to_case_match(&self, dest: &Ident, use_try_from: bool, has_effect: bool) -> TokenStream2 {
        let dest_case = &self.dest.ident;
        let fields = &self.dest.fields;

        let field_suffix = if has_effect {
            quote! { .value }
        } else {
            quote! {}
        };

        match (fields, use_try_from) {
            (Fields::Unit, true) => {
                panic!("multiple source options found for a single destination and the source does not have a field to try_from");
            }
            (Fields::Unit, false) => {
                quote! {
                    #dest::#dest_case
                }
            }
            (Fields::Named(_), _) => {
                let args = self.to_wrapped_args(|id| {
                    quote! {
                        #id: #id#field_suffix
                    }
                });

                quote! {
                    #dest::#dest_case #args
                }
            }
            (Fields::Unnamed(_), _) => {
                let args = self.to_wrapped_args(|id| {
                    quote! {
                        #id#field_suffix
                    }
                });

                quote! {
                    #dest::#dest_case #args
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct SrcCase {
    case_name: Ident,
    fallible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SrcEnum {
    All(),
    Single(Path),
}

type SrcCasesBySrc = HashMap<SrcEnum, Vec<SrcCase>>;

#[derive(Debug, Clone)]
struct CaseCfg {
    dest_case: Ident,

    src_cases_by_src: SrcCasesBySrc,
}

trait MergeIn {
    fn merge_in(&mut self, other: Self);
}

impl<K: Eq + std::hash::Hash, Vs: MergeIn> MergeIn for HashMap<K, Vs> {
    fn merge_in(&mut self, other: Self) {
        other.into_iter().for_each(|(k, vs)| match self.entry(k) {
            Entry::Occupied(mut occ) => {
                occ.get_mut().merge_in(vs);
            }
            Entry::Vacant(vac) => {
                vac.insert(vs);
            }
        });
    }
}

impl<T> MergeIn for Vec<T> {
    fn merge_in(&mut self, other: Self) {
        self.extend(other);
    }
}

trait MatchesIdent {
    fn matches_ident(&self, ident: &str) -> bool;
}

impl MatchesIdent for Path {
    fn matches_ident(&self, target_id: &str) -> bool {
        self.get_ident()
            .map(|id| id.to_string() == target_id)
            .unwrap_or(false)
    }
}

#[derive(Debug)]
struct ParsedEnum {
    src_names: HashSet<Path>,
    effect_holder_name: Option<Path>,
    src_cases_by_src_by_dest: HashMap<Variant, SrcCasesBySrc>,
    dest: Ident,
}

impl ParsedEnum {
    fn conversion_cfgs_by_src_case_by_src(
        &self,
    ) -> HashMap<Path, HashMap<Ident, Vec<ConversionCfg>>> {
        let src_names = &self.src_names;

        self.src_cases_by_src_by_dest.iter().fold(
            HashMap::new(),
            |mut conversion_cfgs_by_src_case_by_src, (dest, src_cases_by_src)| {
                src_cases_by_src.iter().for_each(|(src, src_cases)| {
                    let mut m: HashMap<Path, HashMap<Ident, Vec<ConversionCfg>>> = HashMap::new();
                    let conversion_cfgs_by_src_case = src_cases
                        .iter()
                        .map(|src_case| {
                            (
                                src_case.case_name.clone(),
                                vec![ConversionCfg {
                                    src_case: src_case.clone(),
                                    dest: dest.clone(),
                                }],
                            )
                        })
                        .collect();

                    match src {
                        SrcEnum::Single(src) => {
                            m.insert(src.clone(), conversion_cfgs_by_src_case);
                        }
                        SrcEnum::All() => {
                            for src in src_names {
                                m.insert(src.clone(), conversion_cfgs_by_src_case.clone());
                            }
                        }
                    };
                    conversion_cfgs_by_src_case_by_src.merge_in(m);
                });
                conversion_cfgs_by_src_case_by_src
            },
        )
    }
}

#[derive(Debug, Default)]
struct EnumParser {
    src_names: HashSet<Path>,
    effect_holder_name: Option<Path>,
    src_cases_by_src_by_dest: HashMap<Variant, SrcCasesBySrc>,
    errors: Vec<Error>,
}

impl<'ast> EnumParser {
    fn parse(input: TokenStream2) -> Result<ParsedEnum, Error> {
        let mut parser = EnumParser::default();
        let enm: ItemEnum = parse2(input)?;
        visit_item_enum(&mut parser, &enm);

        if parser.src_names.is_empty() {
            return Err(ParseError::new(
                enm.span(),
                "#[from_enum(Src)] must appear at least once to specify the source enum(s)",
            )
            .into());
        }

        if !parser.errors.is_empty() {
            return Err(parser.errors.into());
        }

        Ok(ParsedEnum {
            src_names: parser.src_names,
            effect_holder_name: parser.effect_holder_name,
            src_cases_by_src_by_dest: parser.src_cases_by_src_by_dest,
            dest: enm.ident.clone(),
        })
    }

    fn parse_from_enum_attr(&mut self, node: &'ast Attribute) {
        if !node.path.matches_ident("from_enum") {
            return;
        }

        match parse2::<FromEnumAttr>(node.tokens.clone()) {
            Ok(from_enum_attr) => {
                self.src_names.extend(from_enum_attr.sources);
                self.effect_holder_name = from_enum_attr.effect;
            }
            Err(err) => {
                self.errors.push(err.into());
            }
        }
    }

    fn parse_from_case_attrs(&self, attrs: &'ast Vec<Attribute>) -> SrcCasesBySrc {
        attrs.iter().fold(HashMap::new(), |mut m, attr| {
            if let Ok(new_attrs) = parse2::<FromCaseAttr>(attr.tokens.clone().into()) {
                let new_src_cases_by_src = new_attrs.into_src_cases_by_src();
                let known_srcs = new_src_cases_by_src
                    .iter()
                    .all(|(src_enum, _)| match src_enum {
                        SrcEnum::All() => true,
                        SrcEnum::Single(ref src_enum) => self.src_names.contains(src_enum),
                    });
                if !known_srcs {
                    panic!("Unknown source enum");
                }

                m.merge_in(new_src_cases_by_src);
            }

            m
        })
    }
}

impl<'ast> Visit<'ast> for EnumParser {
    fn visit_attribute(&mut self, node: &'ast Attribute) {
        self.parse_from_enum_attr(node);
    }

    fn visit_variant(&mut self, node: &'ast Variant) {
        let mut src_cases_by_src = self.parse_from_case_attrs(&node.attrs);
        if src_cases_by_src.is_empty() {
            src_cases_by_src.insert(
                SrcEnum::All(),
                vec![SrcCase {
                    case_name: node.ident.clone(),
                    fallible: false,
                }],
            );
        }

        let mut src_cases_by_src_by_dest = HashMap::new();
        src_cases_by_src_by_dest.insert(node.clone(), src_cases_by_src);
        self.src_cases_by_src_by_dest
            .merge_in(src_cases_by_src_by_dest);
    }
}

#[cfg(test)]
mod enum_parser_tests {
    use super::*;

    #[test]
    fn parse_from_enum_single_src() -> Result<(), Error> {
        let toks = quote! {
            #[from_enum(Src1)]
            enum Dest {
                Case1(),
                Case2(),
            }
        };
        let parser = EnumParser::parse(toks.into())?;

        let src_names = &parser.src_names;
        let assert_has_src_name = |src: &str| {
            assert!(src_names.iter().any(|n| {
                let src = format_ident!("{}", src);
                let lhs = quote! { #n };
                let rhs = quote! { #src };

                lhs.to_string() == rhs.to_string()
            }));
        };

        assert_has_src_name("Src1");
        assert_eq!(parser.effect_holder_name, None);

        Ok(())
    }

    #[test]
    fn parse_from_enum_multiple_srcs() -> Result<(), Error> {
        let toks = quote! {
            #[from_enum(Src1, Src2)]
            enum Dest {
                Case1(),
                Case2(),
            }
        };
        let parser = EnumParser::parse(toks.into())?;

        let src_names = &parser.src_names;
        let assert_has_src_name = |src: &str| {
            assert!(src_names.iter().any(|n| {
                let src = format_ident!("{}", src);
                let lhs = quote! { #n };
                let rhs = quote! { #src };

                lhs.to_string() == rhs.to_string()
            }));
        };

        assert_has_src_name("Src1");
        assert_has_src_name("Src2");

        assert_eq!(parser.effect_holder_name, None);

        Ok(())
    }

    #[test]
    fn parse_from_enum_srcs_and_effects() -> Result<(), Error> {
        let toks = quote! {
            #[from_enum(Src1, effect_container = MyEffect)]
            enum Dest {
                Case1(),
                Case2(),
            }
        };
        let parser = EnumParser::parse(toks.into())?;

        let src_names = &parser.src_names;
        let assert_has_src_name = |src: &str| {
            assert!(src_names.iter().any(|n| {
                let src = format_ident!("{}", src);
                let lhs = quote! { #n };
                let rhs = quote! { #src };

                lhs.to_string() == rhs.to_string()
            }));
        };

        assert_has_src_name("Src1");

        assert_eq!(
            parser
                .effect_holder_name
                .unwrap()
                .get_ident()
                .unwrap()
                .to_string(),
            String::from("MyEffect")
        );

        Ok(())
    }

    #[test]
    fn parse_from_enum_srcs_no_from_enum() -> Result<(), Error> {
        let toks = quote! {
            enum Dest {
                Case1(),
                Case2(),
            }
        };
        let res = EnumParser::parse(toks.into());

        assert!(res.is_err());

        Ok(())
    }

    #[test]
    fn parse_from_enum_srcs_bad_effect() -> Result<(), Error> {
        let toks = quote! {
            #[from_enum(Src1, effect_containerS = MyEffect)]
            enum Dest {
                Case1(),
                Case2(),
            }
        };
        let res = EnumParser::parse(toks.into());

        assert!(res.is_err());

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct CaseMatch {
    src_enum: SrcEnum,
    src_case: SrcCase,
}

impl CaseMatch {
    fn into_src_cases_by_src(self) -> SrcCasesBySrc {
        let mut scbs = HashMap::new();
        scbs.insert(self.src_enum, vec![self.src_case]);
        scbs
    }
}

impl Parse for CaseMatch {
    fn parse(input: ParseStream) -> ParseResult<Self> {
        let parts: Vec<Path> = Punctuated::<Path, Token![=]>::parse_separated_nonempty(input)?
            .into_iter()
            .collect();
        let parts: &[Path] = &parts;

        match parts {
            [src_enum, src_case] => Ok(CaseMatch {
                src_enum: SrcEnum::Single(src_enum.clone()),
                src_case: SrcCase {
                    case_name: src_case.get_ident().unwrap().clone(),
                    fallible: false,
                },
            }),
            [src_case] => Ok(CaseMatch {
                src_enum: SrcEnum::All(),
                src_case: SrcCase {
                    case_name: src_case.get_ident().unwrap().clone(),
                    fallible: false,
                },
            }),
            _ => Err(ParseError::new(
                input.span(),
                "Expected #[from_enum(SrcCase, ..)] or #[from_enum(SrcEnum = SrcCase)]",
            )),
        }
    }
}

#[derive(Debug, Clone)]
struct FromCaseAttr {
    case_matches: Vec<CaseMatch>,
}

impl FromCaseAttr {
    fn into_src_cases_by_src(self) -> SrcCasesBySrc {
        self.case_matches
            .into_iter()
            .fold(HashMap::new(), |mut m, cm| {
                m.merge_in(cm.into_src_cases_by_src());
                m
            })
    }
}

impl Parse for FromCaseAttr {
    fn parse(input: ParseStream) -> ParseResult<Self> {
        let content;
        parenthesized!(content in input);
        let case_matches = Punctuated::<CaseMatch, Token![,]>::parse_terminated(&content)?;
        Ok(FromCaseAttr {
            case_matches: case_matches.into_iter().collect(),
        })
    }
}

#[derive(Debug, Clone)]
struct FromEnumAttr {
    sources: Vec<Path>,
    effect: Option<Path>,
}

impl Parse for FromEnumAttr {
    // parse a stream like (Src1, Src2, e
    fn parse(input: ParseStream) -> ParseResult<Self> {
        let content;
        parenthesized!(content in input);
        let mut sources: Vec<Path> = vec![];
        let mut effect: Option<Path> = None;

        loop {
            let lhs: Path = content.parse()?;
            if content.peek(Token![=]) {
                content.parse::<EqToken>()?; // skip =
                if lhs.get_ident().unwrap().to_string() != "effect_container" {
                    return Err(ParseError::new(
                        lhs.span(),
                        "from_enum only accepts source enums and effect_container = YourEffectContainerImplementingWithEffects",
                    ));
                }

                let rhs: Path = content.parse()?;
                effect.replace(rhs);
            } else {
                sources.push(lhs);
            }

            if content.peek(Token![,]) {
                content.parse::<CommaToken>()?;
            } else {
                return Ok(FromEnumAttr { sources, effect });
            }
        }
    }
}

fn from_enum_internal(input: TokenStream2) -> Result<TokenStream2, Error> {
    let parser = EnumParser::parse(input)?;

    let dest = &parser.dest;
    let effect_holder_name = &parser.effect_holder_name.as_ref();
    let has_effect = effect_holder_name.is_some();
    let conversion_cfgs_by_src_case_by_src = parser.conversion_cfgs_by_src_case_by_src();
    let result_wrapper = |res: Ident| {
        effect_holder_name
            .map(|n| {
                quote! {
                    #n::new(#res, vec![])
                }
            })
            .unwrap_or_else(|| quote! { #res })
    };

    let impls =
        conversion_cfgs_by_src_case_by_src
            .iter()
            .map(|(src_name, conversion_cfgs_by_src_case)| {
                let cases = conversion_cfgs_by_src_case
                    .iter()
                    .map(|(case, conversion_cfgs)| {
                        let use_try_from = conversion_cfgs.len() > 1;
                        let conversions = conversion_cfgs.iter().map(|conversion_cfg| {
                            conversion_cfg.to_case_match(dest, use_try_from, has_effect)
                        });
                        let example_conversion_cfg = conversion_cfgs.first().unwrap();

                        let match_result = if use_try_from {
                            let lhs = example_conversion_cfg.to_args(|arg, _| quote! { Ok(#arg) });
                            let rhs =
                                example_conversion_cfg.to_args(|arg, _| quote! { #arg.try_into() });
                            let res = result_wrapper(format_ident!("value"));
                            let conversions = conversions.map(|c| {
                                quote! {
                                    if let (#lhs) = (#rhs) {
                                        let value = #c;
                                        return #res;
                                    }
                                }
                            });

                            quote! {
                                #(#conversions)*
                                unreachable!();
                            }
                        } else {
                            let lets = example_conversion_cfg.each_arg(|arg, ty| {
                                let full_type = effect_holder_name
                                    .map(|n| {
                                        quote! { #n<#ty> }
                                    })
                                    .unwrap_or_else(|| quote! { #ty });

                                quote! {
                                    let #arg: #full_type = #arg.into();
                                }
                            });
                            let res = result_wrapper(format_ident!("value"));
                            let conversions = conversions.map(|c| {
                                quote! {
                                    #(#lets)*
                                    let value = #c;
                                    #res
                                }
                            });

                            quote! { #(#conversions)* }
                        };

                        let args = example_conversion_cfg.to_wrapped_args(|arg| quote! { #arg });

                        quote! {
                            #src_name::#case #args => {
                                #match_result
                            }
                        }
                    });
                let dest = effect_holder_name
                    .map(|effect_holder| quote! { #effect_holder<#dest> })
                    .unwrap_or_else(|| quote! { #dest });

                quote! {
                    impl std::convert::From<#src_name> for #dest {
                        fn from(src: #src_name) -> #dest {
                            use std::convert::Into;
                            use std::convert::TryInto;

                            match src {
                                #(#cases),*
                            }
                        }
                    }
                }
            });

    Ok(quote! {
        #(#impls)*
    })
}

#[derive(Debug, Clone)]
enum Error {
    SynError(SynError),
    CompoundError(Vec<Error>),
}

impl Error {
    pub fn into_compile_errors(self) -> Vec<TokenStream2> {
        match self {
            Self::SynError(x) => vec![x.into_compile_error()],
            Self::CompoundError(x) => x
                .into_iter()
                .flat_map(|e| match e {
                    Self::SynError(s) => vec![s.into_compile_error()],
                    Self::CompoundError(_) => e.into_compile_errors(),
                })
                .collect(),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SynError(x) => x.source(),
            Self::CompoundError(x) => x.first().and_then(std::error::Error::source),
        }
    }
}

impl From<SynError> for Error {
    fn from(syn_error: SynError) -> Error {
        Error::SynError(syn_error)
    }
}

impl From<Vec<Error>> for Error {
    fn from(errors: Vec<Error>) -> Error {
        Error::CompoundError(errors)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{:?}", self)
    }
}
