use crate::error::Error;
use crate::ir::{ConversionCfg, SrcCase, SrcCasesBySrc, SrcEnum};
use crate::merge_in::MergeIn;
use crate::syn_extensions::MatchesIdent;
use proc_macro2::TokenStream as TokenStream2;
use std::collections::{HashMap, HashSet};
use syn::{
    parenthesized,
    parse::{Error as ParseError, Parse, ParseStream, Result as ParseResult},
    parse2,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Comma as CommaToken, Eq as EqToken},
    visit::{visit_item_enum, Visit},
    Attribute, Ident, ItemEnum, Path, Token, Variant,
};

#[derive(Debug)]
pub struct ParsedEnum {
    pub dest: Ident,
    pub effect_holder_name: Option<Path>,
    src_names: HashSet<Path>,
    src_cases_by_src_by_dest: HashMap<Variant, SrcCasesBySrc>,
    dest_case_order: HashMap<Variant, usize>,
}

impl ParsedEnum {
    pub fn conversion_cfgs_by_src_case_by_src(
        &self,
    ) -> HashMap<Path, HashMap<Ident, Vec<ConversionCfg>>> {
        let src_names = &self.src_names;

        let conversion_cfgs_by_src_case_by_src = self.src_cases_by_src_by_dest.iter().fold(
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
        );
        conversion_cfgs_by_src_case_by_src
            .into_iter()
            .map(|(src, conversion_cfgs_by_src_case)| {
                (
                    src,
                    conversion_cfgs_by_src_case
                        .into_iter()
                        .map(|(src_case, mut conversion_cfgs)| {
                            conversion_cfgs.sort_by_key(|cfg| {
                                self.dest_case_order.get(&cfg.dest).unwrap_or(&0)
                            });
                            (src_case, conversion_cfgs)
                        })
                        .collect(),
                )
            })
            .collect()
    }
}

#[derive(Debug, Default)]
pub struct EnumParser {
    src_names: HashSet<Path>,
    effect_holder_name: Option<Path>,
    src_cases_by_src_by_dest: HashMap<Variant, SrcCasesBySrc>,
    dest_case_order: HashMap<Variant, usize>,
    errors: Vec<Error>,
}

impl<'ast> EnumParser {
    pub fn parse(input: TokenStream2) -> Result<ParsedEnum, Error> {
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
            dest: enm.ident,
            dest_case_order: parser.dest_case_order,
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
        self.dest_case_order
            .insert(node.clone(), self.dest_case_order.len());
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
    use quote::{format_ident, quote};

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
