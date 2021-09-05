extern crate proc_macro;

mod error;
mod ir;
mod merge_in;
mod parser;
mod syn_extensions;

use crate::error::Error;
use crate::ir::ConversionCfg;
use crate::parser::EnumParser;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::fs::File;

#[proc_macro_derive(FromEnum, attributes(from_enum, from_case))]
pub fn derive_enum_from(input: TokenStream) -> TokenStream {
    let result = from_enum_internal(input.into()).unwrap_or_else(|err| {
        let errors = err.into_compile_errors();
        quote! {
            #(#errors)*
        }
    });

    let mut file = File::create("output.rs").expect("failed to create file");
    std::io::Write::write_all(&mut file, result.to_string().as_bytes()).expect("failed to write");

    result.into()
}

fn from_enum_internal(input: TokenStream2) -> Result<TokenStream2, Error> {
    let parser = EnumParser::parse(input)?;

    let dest = &parser.dest;
    let effect_holder_name = &parser.effect_holder_name.as_ref();
    let has_effect = effect_holder_name.is_some();
    let conversion_cfgs_by_src_case_by_src = parser.conversion_cfgs_by_src_case_by_src();
    let result_wrapper =
        |case_match: TokenStream2, conversion_cfg: &ConversionCfg, should_return: bool| {
            let ret = if should_return {
                quote! { return }
            } else {
                quote! {}
            };

            effect_holder_name
            .map(|n| {
                let chains = conversion_cfg.to_args(|arg, _| {
                    let arg_effects = format_ident!("{}_effects", arg);
                    quote! { .chain(#arg_effects) }
                });
                let vals_and_effects = conversion_cfg.each_arg(|arg, _| {
                    let arg_val = format_ident!("{}_value", arg);
                    let arg_effects = format_ident!("{}_effects", arg);
                    quote! {
                        let (#arg_val, #arg_effects) = #arg.into_value_and_effects();
                    }
                });
                quote! {
                    #(#vals_and_effects)*
                    let value = #case_match;
                    let effects = std::iter::empty()#chains.collect::<Vec<_>>().into_boxed_slice();

                    #ret #n::compose_from(value, effects)
                }
            })
            .unwrap_or_else(|| quote! { #ret #case_match})
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
                            let case_match =
                                conversion_cfg.to_case_match(dest, use_try_from, has_effect);

                            if use_try_from {
                                let arg_let = conversion_cfg.each_arg(|arg, ty| {
                                let arg_res = format_ident!("{}_res", &arg);
                                let typ = effect_holder_name
                                    .map(|n| {
                                        quote! { #n<#ty> }
                                    })
                                    .unwrap_or_else(|| quote! { #ty });

                                quote! {
                                    let #arg_res: std::result::Result<#typ, _> = #arg.try_into();
                                }
                            });
                                let lhs = conversion_cfg.to_args(|arg, _| quote! { Ok(#arg) });
                                let rhs = conversion_cfg.to_args(|arg, _| {
                                    let arg_res = format_ident!("{}_res", &arg);
                                    quote! { #arg_res }
                                });
                                let res = result_wrapper(case_match, conversion_cfg, true);

                                quote! {
                                    #(#arg_let)*
                                    if let (#lhs) = (#rhs) {
                                        #res;
                                    }
                                }
                            } else {
                                let lets = conversion_cfg.each_arg(|arg, ty| {
                                    let full_type = effect_holder_name
                                        .map(|n| {
                                            quote! { #n<#ty> }
                                        })
                                        .unwrap_or_else(|| quote! { #ty });

                                    quote! {
                                        let #arg: #full_type = #arg.into();
                                    }
                                });
                                let res = result_wrapper(case_match, conversion_cfg, false);
                                quote! {
                                    #(#lets)*
                                    #res
                                }
                            }
                        });

                        let example_conversion_cfg = conversion_cfgs.first().unwrap();

                        let args = example_conversion_cfg.to_wrapped_args(|arg| quote! { #arg });
                        let trailer = if use_try_from {
                            quote! {
                                unreachable!();
                            }
                        } else {
                            quote! {}
                        };

                        quote! {
                            #src_name::#case #args => {
                                #(#conversions)*
                                #trailer
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
