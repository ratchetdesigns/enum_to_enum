extern crate proc_macro;

use std::fmt::{Display, Formatter, Result as FmtResult};
use std::convert::From;
use proc_macro::TokenStream;
use syn::{parse, ItemEnum, Attribute,
          Meta, MetaList, NestedMeta, Path,
          parse::{Error as ParseError},
          visit::{Visit, visit_item_enum}};
use quote::quote;

#[proc_macro_derive(FromEnum, attributes(from_enum, from_case))]
pub fn derive_enum_from(input: TokenStream) -> TokenStream {
    from_enum_internal(input)
        .unwrap_or_else(|_err| {
            panic!("Failed to parse input");
        })
}

#[derive(Debug, Default)]
struct EnumParser {
    src_names: Vec<Path>,
}

impl<'ast> Visit<'ast> for EnumParser {
    fn visit_attribute(&mut self, node: &'ast Attribute) {
        self.src_names.extend::<Vec<Path>>(
            node.parse_meta()
                .map(|meta| match meta {
                    Meta::List(MetaList { path, nested, .. }) => {
                        let is_from_enum = path.get_ident()
                            .map(|id| id.to_string() == "from_enum")
                            .unwrap_or(false);
                        if !is_from_enum {
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
