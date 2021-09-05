use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{Fields, Ident, Path, Type, Variant};

#[derive(Debug, Clone)]
pub struct ConversionCfg {
    pub src_case: SrcCase,
    pub dest: Variant,
}

impl ConversionCfg {
    pub fn each_arg<F: Fn(&Ident, &Type) -> TokenStream2>(&self, xform: F) -> Vec<TokenStream2> {
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

    pub fn to_args<T: Fn(&Ident, &Type) -> TokenStream2>(&self, xform: T) -> TokenStream2 {
        let args = self.each_arg(xform);
        quote! {
            #(#args),*
        }
    }

    pub fn to_wrapped_args<T: Fn(&Ident) -> TokenStream2>(&self, xform: T) -> TokenStream2 {
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

    pub fn to_case_match(
        &self,
        dest: &Ident,
        use_try_from: bool,
        has_effect: bool,
    ) -> TokenStream2 {
        let dest_case = &self.dest.ident;
        let fields = &self.dest.fields;

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
                    if has_effect {
                        let id_val = format_ident!("{}_value", id);
                        quote! {
                            #id: #id_val
                        }
                    } else {
                        quote! {
                            #id
                        }
                    }
                });

                quote! {
                    #dest::#dest_case #args
                }
            }
            (Fields::Unnamed(_), _) => {
                let args = self.to_wrapped_args(|id| {
                    if has_effect {
                        let id_val = format_ident!("{}_value", id);
                        quote! {
                            #id_val
                        }
                    } else {
                        quote! {
                            #id
                        }
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
pub struct SrcCase {
    pub case_name: Ident,
    pub fallible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SrcEnum {
    All(),
    Single(Path),
}

pub type SrcCasesBySrc = HashMap<SrcEnum, Vec<SrcCase>>;
