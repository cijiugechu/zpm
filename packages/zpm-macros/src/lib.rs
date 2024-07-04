extern crate proc_macro;

use quote::{format_ident, quote};
use syn::{meta::ParseNestedMeta, parse_macro_input, Data, DeriveInput, Expr, ItemFn, Meta};

#[proc_macro_attribute]
pub fn track_time(_attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    // Decompose the input function to inspect its components
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = input_fn;

    let fn_name = &sig.ident; // Get the function name
    let is_async = sig.asyncness.is_some();
    let result_var = format_ident!("result");

    let exec_time_log = quote! {
        let duration = start.elapsed();
        println!("{} took {:?}", stringify!(#fn_name), duration);
    };

    // Apply different logic based on the async attribute
    let output = if is_async {
        quote! {
            #(#attrs)* #vis #sig {
                let start = std::time::Instant::now();
                let #result_var = (|| async #block)().await;
                #exec_time_log
                #result_var
            }
        }
    } else {
        quote! {
            #(#attrs)* #vis #sig {
                let start = std::time::Instant::now();
                let #result_var = (|| #block)();
                #exec_time_log
                #result_var
            }
        }
    };

    output.into()
}

fn extract_literal(meta: &ParseNestedMeta) -> syn::Result<String> {
    let expr: syn::Expr = meta.value()?.parse()?;
    let value = &expr;

    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(lit),
        attrs: _,
    }) = value {
        return Ok(lit.value());
    }

    panic!("Invalid syntax")
}

fn extract_bool(meta: &ParseNestedMeta) -> syn::Result<bool> {
    let expr: syn::Expr = meta.value()?.parse()?;
    let value = &expr;

    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Bool(lit),
        attrs: _,
    }) = value {
        return Ok(lit.value);
    }

    panic!("Invalid syntax")
}

#[proc_macro_derive(Parsed, attributes(parse_error, try_from_str, try_pattern))]
pub fn parsed_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let user_error = ast.attrs.iter().find_map(|attr| {
        if let Meta::List(list) = &attr.meta {
            if list.path.is_ident("parse_error") {
                return Some(list.tokens.clone());
            }
        }
        None
    });

    let error_value = user_error.unwrap_or(quote!{
    });

    let name = &ast.ident;
    let data = match &ast.data {
        Data::Enum(data) => data,
        _ => panic!("Parsed can only be derived for enums"),
    };

    #[derive(Debug)]
    struct Variant {
        ident: syn::Ident,
        prefix: Option<String>,
        pattern: Option<String>,
        optional_prefix: bool,
        field_count: usize,
    }

    let mut variants: Vec<Variant> = Vec::new();
    let mut arms = Vec::new();

    for variant in &data.variants {
        variants.extend(variant.attrs.iter().filter_map(|attr| {
            attr.path().is_ident("try_pattern").then(|| {
                let mut variant_info = Variant {
                    ident: variant.ident.clone(),
                    prefix: None,
                    pattern: None,
                    optional_prefix: false,
                    field_count: variant.fields.len(),
                };
        
                if let Ok(_) = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("prefix") {
                        variant_info.prefix = extract_literal(&meta).ok();
                    }

                    if meta.path.is_ident("pattern") {
                        variant_info.pattern = extract_literal(&meta).ok();
                    }

                    if meta.path.is_ident("optional_prefix") {
                        variant_info.optional_prefix = extract_bool(&meta).unwrap_or_default();
                    }

                    Ok(())
                }) {}

                // For some reason, performing the starts_with check is twice slower than doing
                // the regex match during resolution. So we just bake the prefix into the pattern.
                if let Some(prefix) = variant_info.prefix.take() {
                    let prefix_part = if variant_info.optional_prefix {
                        format!("(?:{})?", regex::escape(&prefix))
                    } else {
                        regex::escape(&prefix)
                    };

                    let pattern_part = if let Some(pattern) = variant_info.pattern.take() {
                        pattern
                    } else {
                        "(.*)".to_string()
                    };

                    variant_info.pattern = Some(format!("{}{}", prefix_part, pattern_part));
                }

                if let Some(pattern) = variant_info.pattern.take() {
                    variant_info.pattern = Some(format!("^{}$", pattern));
                }

                variant_info
            })
        }));
    }

    for variant in &variants {
        let variant_name = &variant.ident;

        let enum_args = if let Some(pattern) = &variant.pattern {
            let captures_len = regex::Regex::new(&pattern)
                .unwrap()
                .captures_len();

            (1..captures_len).map(|index| quote! {
                captures.get(#index).unwrap().as_str().try_into().map_err(|_| ())?
            }).collect::<Vec<_>>()
        } else if variant.field_count > 0 {
            vec![quote!{ src.try_into().map_err(|_| ())? }]
        } else {
            vec![]
        };

        let mut arm = quote! {
            if let Ok(val) = (|| -> Result<Self, ()> { Ok(Self::#variant_name(#(#enum_args),*)) })() {
                return Ok(val);
            }
        };

        if let Some(pattern) = &variant.pattern {
            arm = quote! {
                static RE: once_cell::sync::Lazy<regex::Regex>
                    = once_cell::sync::Lazy::new(|| regex::Regex::new(#pattern).unwrap());

                if let Some(captures) = RE.captures(src) {
                    #arm
                }
            };
        }

        if let Some(prefix) = &variant.prefix {
            if variant.optional_prefix {
                arm = quote! {
                    if src.starts_with(#prefix) {
                        let src = &src[#prefix.len()..];
                        #arm
                    } else {
                        #arm
                    }
                };
            } else {
                arm = quote! {
                    if src.starts_with(#prefix) {
                        let src = &src[#prefix.len()..];
                        #arm
                    }
                };
            }
        }

        arms.push(quote! { {
            #arm
        } });
    }

    let expanded = quote! {
        crate::yarn_serialization_protocol!(#name, {
            deserialize(src) {
                #(#arms)*
                Err(#error_value(src.to_string()))
            }
        });
    };

    //panic!("{:?}", expanded.to_string());

    expanded.into()
}

#[proc_macro_attribute]
pub fn yarn_config(_attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let struct_name = &input.ident;

    let fields = if let syn::Data::Struct(data_struct) = &input.data {
        &data_struct.fields
    } else {
        return proc_macro::TokenStream::from(quote! {
            compile_error!("env_default can only be used with structs");
        });
    };

    let mut default_functions = vec![];
    let mut new_fields = vec![];

    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;

        let mut default_value = None;
        let mut array = false;
        let mut nullable = false;

        for attr in &field.attrs {
            if attr.path().is_ident("array") {
                array = true;
            }

            if attr.path().is_ident("default") {
                if let Ok(value) = attr.parse_args::<Expr>() {
                    default_value = Some(value);
                }
            }

            if attr.path().is_ident("nullable") {
                nullable = true;
            }
        }

        let computed_field_type = if array {
            quote! { crate::config::Setting<Vec<#field_type>> }
        } else if nullable {
            quote! { crate::config::Setting<Option<#field_type>> }
        } else {
            quote! { crate::config::Setting<#field_type> }
        };

        if let Some(default) = default_value {
            let value_from_env = if array {
                quote! {
                    value.split(',')
                        .map(|v| #field_type::hydrate_setting_from_env(v.trim().as_str()).unwrap())
                        .collect()
                }
            } else if nullable {
                quote! {
                    if value.is_empty() {
                        None
                    } else {
                        Some(#field_type::hydrate_setting_from_env(value.as_str()).unwrap())
                    }
                }
            } else {
                quote! {
                    #field_type::hydrate_setting_from_env(value.as_str()).unwrap()
                }
            };

            let func_name = syn::Ident::new(&format!("{}_default_from_env", field_name), field_name.span());
            let func_name_str = format!("{}", func_name.to_string());

            default_functions.push(quote! {
                fn #func_name() -> #computed_field_type {
                    match std::env::var(concat!("YARN_", stringify!(#field_name)).to_uppercase()) {
                        Ok(value) => crate::config::Setting {value: #value_from_env, source: crate::config::SettingSource::Env},
                        Err(_) => crate::config::Setting {value: #default, source: crate::config::SettingSource::Default},
                    }
                }
            });

            new_fields.push(quote! {
                #[serde(default = #func_name_str)]
                pub #field_name: #computed_field_type,
            });
        } else {
            if array {
                new_fields.push(quote! {
                    #[serde(default)]
                    pub #field_name: #computed_field_type,
                });
            } else {
                new_fields.push(quote! {
                    pub #field_name: #computed_field_type,
                });
            }
        }
    }

    let expanded = quote! {
        #(#default_functions)*

        #[derive(Clone, Debug, serde::Deserialize)]
        pub struct #struct_name {
            #(#new_fields)*
        }
    };

    // panic!("{:?}", expanded.to_string());
    expanded.into()
}
