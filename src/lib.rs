extern crate proc_macro;

use std::fmt::{Display, Formatter, Result as FmtResult};
use std::collections::{HashMap, hash_map::Entry};
use std::convert::From;
use proc_macro::TokenStream;
use syn::{parse, parse2, parenthesized,
          ItemEnum, Attribute, Meta, MetaList, NestedMeta, Path, Variant, Ident, Token,
          punctuated::Punctuated,
          parse::{Error as ParseError, Parse, ParseStream, Result as ParseResult},
          visit::{Visit, visit_item_enum}};
use quote::{quote, format_ident};

#[proc_macro_derive(FromEnum, attributes(from_enum, from_case))]
pub fn derive_enum_from(input: TokenStream) -> TokenStream {
    from_enum_internal(input)
        .unwrap_or_else(|_err| {
            panic!("Failed to parse input");
        })
}

#[derive(Debug, Clone)]
struct SrcCase {
    case_name: Ident,
    fallible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SrcEnum {
    All(),
    Single(Path)
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

impl<K: Eq + std::hash::Hash, V> MergeIn for HashMap<K, Vec<V>> {
    fn merge_in(&mut self, other: Self) {
        other.into_iter()
            .for_each(|(k, vs)| {
                match self.entry(k) {
                    Entry::Occupied(mut occ) => {
                        occ.get_mut().extend(vs);
                    },
                    Entry::Vacant(vac) => {
                        vac.insert(vs);
                    },
                }
            });
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

#[derive(Debug, Default)]
struct EnumParser {
    src_names: Vec<Path>,
}

impl<'ast> EnumParser {
    fn parse_from_enum_attr(&mut self, node: &'ast Attribute) {
        self.src_names.extend::<Vec<Path>>(
            node.parse_meta()
                .map(|meta| match meta {
                    Meta::List(MetaList { path, nested, .. }) => {
                        if !path.matches_ident("from_enum") {
                            return Default::default();
                        }

                        nested.into_iter()
                            .filter_map(|m| match m {
                                NestedMeta::Meta(m) => match m {
                                    Meta::Path(p) => Some(p),
                                    _ => None,
                                },
                                _ => None,
                            })
                            .collect()
                    },
                    _ => Default::default()
                })
                .unwrap_or_default()
        );
    }

    fn parse_from_case_attrs(&self, attrs: &'ast Vec<Attribute>) -> SrcCasesBySrc {
        attrs.iter()
            .fold(
                HashMap::new(),
                |mut m, attr| {
                    println!("ATTR: {:?}, META: {:?}", attr, attr.parse_meta());
                    let new_attrs = parse2::<FromCaseAttr>(attr.tokens.clone().into());
                    if new_attrs.is_ok() {
                        m.merge_in(new_attrs.unwrap().into_src_cases_by_src());
                    }

                    m
                }
            )
    }
}

impl<'ast> Visit<'ast> for EnumParser {
    fn visit_attribute(&mut self, node: &'ast Attribute) {
        self.parse_from_enum_attr(node);
    }

    fn visit_variant(&mut self, node: &'ast Variant) {
        let src_cases_by_src = self.parse_from_case_attrs(&node.attrs);
        println!("SRC CASES! {:?}", src_cases_by_src);
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
        scbs.insert(
            self.src_enum,
            vec![self.src_case],
        );
        scbs
    }
}

impl Parse for CaseMatch {
    fn parse(input: ParseStream) -> ParseResult<Self> {
        let parts: Vec<Path> = Punctuated::<Path, Token![=]>::parse_terminated(input)?.into_iter().collect();
        let parts: &[Path] = &parts;

        match parts {
            [src_enum, src_case] => Ok(CaseMatch {
                src_enum: SrcEnum::Single(src_enum.clone()),
                src_case: SrcCase {
                    case_name: src_case.get_ident().unwrap().clone(),
                    fallible: false,
                }
            }),
            [src_case] => Ok(CaseMatch {
                src_enum: SrcEnum::All(),
                src_case: SrcCase {
                    case_name: src_case.get_ident().unwrap().clone(),
                    fallible: false,
                }
            }),
            _ => Err(ParseError::new(input.span(), "Expected #[from_enum(SrcCase, ..)] or #[from_enum(SrcEnum = SrcCase)]")),
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
            .fold(
                HashMap::new(),
                |mut m, cm| {
                    m.merge_in(cm.into_src_cases_by_src());
                    m
                }
            )
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

fn from_enum_internal(input: TokenStream) -> Result<TokenStream, Error> {
    let enm: ItemEnum = parse(input)?;
    let mut parser = EnumParser::default();
    visit_item_enum(&mut parser, &enm);
    if parser.src_names.is_empty() {
        panic!("#[from_enum(Src)] must appear at least once to specify the source enum");
    }

    let dest = &enm.ident;
    let impls = parser.src_names
        .iter()
        .map(|src_name| quote! {
            impl std::convert::From<#src_name> for #dest {
                fn from(src: #src_name) -> #dest {
                    #dest::Case1()
                }
            }
        });

    Ok(quote! {
        #(#impls)*
    }.into())
}

#[derive(Debug, Clone)]
enum Error {
    ParseError(ParseError),
}

impl From<ParseError> for Error {
    fn from(parse_error: ParseError) -> Error {
        Error::ParseError(parse_error)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{:?}", self)
    }
}
