#![deny(unsafe_code)]
//! A collection of helper types and functions for working on macros within the Bevy ecosystem.

extern crate proc_macro;
#[macro_use]
extern crate lazy_static;

mod label;
mod manifest;


use std::sync::{Arc, Mutex};

use label::derive_label;
use manifest::Manifest;
use proc_macro::{Span, TokenStream, TokenTree};
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput};

use rustc_hash::FxHashSet;
use syn::{
    parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token::Comma,
    ConstParam, Data, DataStruct, Error, Fields, FieldsNamed, GenericParam, Ident,
    Index, TypeParam,
};

/// Derive macro generating an impl of the trait `StageLabel`.
///
/// This does not work for unions.
#[proc_macro_derive(StageLabel)]
pub fn derive_stage_label(input: TokenStream) -> TokenStream {
    derive_label_inner(input, "StageLabel")
}

#[proc_macro_derive(ScheduleLabel)]
pub fn derive_schedule_label(input: TokenStream) -> TokenStream {
    derive_label_inner(input, "ScheduleLabel")
}

#[proc_macro_derive(SystemSet)]
pub fn derive_system_set(input: TokenStream) -> TokenStream {
    derive_label_inner(input, "SystemSet")
}

#[proc_macro_derive(Resource)]
pub fn derive_resource(_input: TokenStream) -> TokenStream {
    // component::derive_resource(input)
    TokenStream::from(quote! {})
}

#[proc_macro_derive(Component, attributes(component))]
pub fn derive_component(_input: TokenStream) -> TokenStream {
    TokenStream::from(quote! {})
}
/// Implement `SystemParam` to use a struct as a parameter in a system
#[proc_macro_derive(SystemParam, attributes(system_param))]
pub fn derive_system_param(input: TokenStream) -> TokenStream {
    let token_stream = input.clone();
    let ast = parse_macro_input!(input as DeriveInput);
    let syn::Data::Struct(syn::DataStruct {
        fields: field_definitions,
        ..
    }) = ast.data
    else {
        return syn::Error::new(
            ast.span(),
            "Invalid `SystemParam` type: expected a `struct`",
        )
        .into_compile_error()
        .into();
    };
    let path = ecs_path();

    let mut field_locals = Vec::new();
    let mut fields = Vec::new();
    let mut field_types = Vec::new();
    for (i, field) in field_definitions.iter().enumerate() {
        field_locals.push(format_ident!("f{i}"));
        let i = Index::from(i);
        fields.push(
            field
                .ident
                .as_ref()
                .map(|f| quote! { #f })
                .unwrap_or_else(|| quote! { #i }),
        );
        field_types.push(&field.ty);
    }

    let generics = ast.generics;

    // Emit an error if there's any unrecognized lifetime names.
    for lt in generics.lifetimes() {
        let ident = &lt.lifetime.ident;
        let w = format_ident!("w");
        let s = format_ident!("s");
        if ident != &w && ident != &s {
            return syn::Error::new_spanned(
                lt,
                r#"invalid lifetime name: expected `'w` or `'s`
 'w -- refers to data stored in the World.
 's -- refers to data stored in the SystemParam's state.'"#,
            )
            .into_compile_error()
            .into();
        }
    }

    let (_impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let lifetimeless_generics: Vec<_> = generics
        .params
        .iter()
        .filter(|g| !matches!(g, GenericParam::Lifetime(_)))
        .collect();

    let shadowed_lifetimes: Vec<_> = generics.lifetimes().map(|_| quote!('_)).collect();

    let mut punctuated_generics = Punctuated::<_, Comma>::new();
    punctuated_generics.extend(lifetimeless_generics.iter().map(|g| match g {
        GenericParam::Type(g) => GenericParam::Type(TypeParam {
            default: None,
            ..g.clone()
        }),
        GenericParam::Const(g) => GenericParam::Const(ConstParam {
            default: None,
            ..g.clone()
        }),
        _ => unreachable!(),
    }));

    let mut punctuated_generic_idents = Punctuated::<_, Comma>::new();
    punctuated_generic_idents.extend(lifetimeless_generics.iter().map(|g| match g {
        GenericParam::Type(g) => &g.ident,
        GenericParam::Const(g) => &g.ident,
        _ => unreachable!(),
    }));

    let punctuated_generics_no_bounds: Punctuated<_, Comma> = lifetimeless_generics
        .iter()
        .map(|&g| match g.clone() {
            GenericParam::Type(mut g) => {
                g.bounds.clear();
                GenericParam::Type(g)
            }
            g => g,
        })
        .collect();

    let mut tuple_types: Vec<_> = field_types.iter().map(|x| quote! { #x }).collect();
    let mut tuple_patterns: Vec<_> = field_locals.iter().map(|x| quote! { #x }).collect();

    // If the number of fields exceeds the 16-parameter limit,
    // fold the fields into tuples of tuples until we are below the limit.
    const LIMIT: usize = 16;
    while tuple_types.len() > LIMIT {
        let end = Vec::from_iter(tuple_types.drain(..LIMIT));
        tuple_types.push(parse_quote!( (#(#end,)*) ));

        let end = Vec::from_iter(tuple_patterns.drain(..LIMIT));
        tuple_patterns.push(parse_quote!( (#(#end,)*) ));
    }

    // Create a where clause for the `ReadOnlySystemParam` impl.
    // Ensure that each field implements `ReadOnlySystemParam`.
    let mut read_only_generics = generics.clone();
    let read_only_where_clause = read_only_generics.make_where_clause();
    for field_type in &field_types {
        read_only_where_clause
            .predicates
            .push(syn::parse_quote!(#field_type: #path::system::ReadOnlySystemParam));
    }
    let struct_name = &ast.ident;

    let fields_alias =
        ensure_no_collision(format_ident!("__{}StructFieldsAlias", struct_name), token_stream.clone());

    
    let state_struct_visibility = &ast.vis;
    let state_struct_name = ensure_no_collision(format_ident!("{}FetchState", struct_name), token_stream);

    TokenStream::from(quote! {
        // We define the FetchState struct in an anonymous scope to avoid polluting the user namespace.
        // The struct can still be accessed via SystemParam::State, e.g. EventReaderState can be accessed via
        // <EventReader<'static, 'static, T> as SystemParam>::State
        // const _: () = {
            // Allows rebinding the lifetimes of each field type.
            type #fields_alias <'w, #punctuated_generics_no_bounds> = (#(#tuple_types,)*);

            #[doc(hidden)]
            #state_struct_visibility struct #state_struct_name <#(#lifetimeless_generics,)*>
            #where_clause {
                state: <#fields_alias::<'static, #punctuated_generic_idents> as #path::prelude::SystemParam>::State,
            }

         impl<#punctuated_generics> #path::prelude::SystemParam for
                #struct_name <#(#shadowed_lifetimes,)* #punctuated_generic_idents> #where_clause
            {
                type State = #state_struct_name<#punctuated_generic_idents>;
                type Item<'w> = #struct_name #ty_generics;

                fn init_state(world: &mut #path::world::World, system_meta: &mut #path::system::SystemMeta) -> Self::State {
                    #state_struct_name {
                        state: <#fields_alias::<'_, #punctuated_generic_idents> as #path::prelude::SystemParam>::init_state(world, system_meta),
                    }
                }

                fn archetype_depend<'w>(
                    world: & #path::world::World,
                    system_meta: & #path::system::SystemMeta,
                    state: &Self::State,
                    archetype: & #path::archetype::Archetype,
                    depend: & mut #path::archetype::ArchetypeDependResult,
                ) {
                    <(#(#tuple_types,)*) as #path::prelude::SystemParam>::archetype_depend(world, system_meta, &state.state, archetype, depend);
                }

                fn res_depend<'w>(
                    world: &'w #path::world::World,
                    system_meta: &'w #path::system::SystemMeta,
                    state: &'w Self::State,
                    res_tid: &'w std::any::TypeId,
                    res_name: &'w std::borrow::Cow<'static, str>,
                    single: bool,
                    result: &'w mut #path::archetype::Flags,
                ) {
                    <(#(#tuple_types,)*) as #path::prelude::SystemParam>::res_depend(world, system_meta, &state.state, res_tid, res_name, single, result);
                }

                fn get_param<'w>(
                    world: &'w #path::world::World,
                    system_meta: &'w #path::system::SystemMeta,
                    state: &'w mut Self::State,
                ) -> Self::Item<'w> {
                    let (#(#tuple_patterns,)*) = <(#(#tuple_types,)*) as #path::prelude::SystemParam>::get_param(world, system_meta, &mut state.state);
                    #struct_name {
                        #(#fields: #field_locals,)*
                    }
                    // todo!()
                }

                fn get_self<'w>(
                    world: &'w #path::world::World,
                    system_meta: &'w #path::system::SystemMeta,
                    state: &'w mut Self::State,
                ) -> Self {
                    unsafe { std::mem::transmute(Self::get_param(world, system_meta, state)) }
                }
            }
            // Safety: Each field is `ReadOnlySystemParam`, so this can only read from the `World`
            // unsafe impl<'w, 's, #punctuated_generics> #path::system::ReadOnlySystemParam for #struct_name #ty_generics #read_only_where_clause {}
        // };
    })
}

/// Implement `SystemParam` to use a struct as a parameter in a system
#[proc_macro_derive(ParamSetElement, attributes(param_set_element))]
pub fn derive_param_set_element(input: TokenStream) -> TokenStream {
    let token_stream = input.clone();
    let ast = parse_macro_input!(input as DeriveInput);
    let syn::Data::Struct(syn::DataStruct {
        fields: field_definitions,
        ..
    }) = ast.data
    else {
        return syn::Error::new(
            ast.span(),
            "Invalid `SystemParam` type: expected a `struct`",
        )
        .into_compile_error()
        .into();
    };
    let path = ecs_path();

    let mut field_locals = Vec::new();
    let mut fields = Vec::new();
    let mut field_types = Vec::new();
    for (i, field) in field_definitions.iter().enumerate() {
        field_locals.push(format_ident!("f{i}"));
        let i = Index::from(i);
        fields.push(
            field
                .ident
                .as_ref()
                .map(|f| quote! { #f })
                .unwrap_or_else(|| quote! { #i }),
        );
        field_types.push(&field.ty);
    }

    let generics = ast.generics;

    // Emit an error if there's any unrecognized lifetime names.
    for lt in generics.lifetimes() {
        let ident = &lt.lifetime.ident;
        let w = format_ident!("w");
        let s = format_ident!("s");
        if ident != &w && ident != &s {
            return syn::Error::new_spanned(
                lt,
                r#"invalid lifetime name: expected `'w` or `'s`
 'w -- refers to data stored in the World.
 's -- refers to data stored in the SystemParam's state.'"#,
            )
            .into_compile_error()
            .into();
        }
    }

    let (_impl_generics, _ty_generics, where_clause) = generics.split_for_impl();

    let lifetimeless_generics: Vec<_> = generics
        .params
        .iter()
        .filter(|g| !matches!(g, GenericParam::Lifetime(_)))
        .collect();

    let shadowed_lifetimes: Vec<_> = generics.lifetimes().map(|_| quote!('_)).collect();

    let mut punctuated_generics = Punctuated::<_, Comma>::new();
    punctuated_generics.extend(lifetimeless_generics.iter().map(|g| match g {
        GenericParam::Type(g) => GenericParam::Type(TypeParam {
            default: None,
            ..g.clone()
        }),
        GenericParam::Const(g) => GenericParam::Const(ConstParam {
            default: None,
            ..g.clone()
        }),
        _ => unreachable!(),
    }));

    let mut punctuated_generic_idents = Punctuated::<_, Comma>::new();
    punctuated_generic_idents.extend(lifetimeless_generics.iter().map(|g| match g {
        GenericParam::Type(g) => &g.ident,
        GenericParam::Const(g) => &g.ident,
        _ => unreachable!(),
    }));

    let mut tuple_types: Vec<_> = field_types.iter().map(|x| quote! { #x }).collect();
    let mut tuple_patterns: Vec<_> = field_locals.iter().map(|x| quote! { #x }).collect();

    // If the number of fields exceeds the 16-parameter limit,
    // fold the fields into tuples of tuples until we are below the limit.
    const LIMIT: usize = 16;
    while tuple_types.len() > LIMIT {
        let end = Vec::from_iter(tuple_types.drain(..LIMIT));
        tuple_types.push(parse_quote!( (#(#end,)*) ));

        let end = Vec::from_iter(tuple_patterns.drain(..LIMIT));
        tuple_patterns.push(parse_quote!( (#(#end,)*) ));
    }

    // Create a where clause for the `ReadOnlySystemParam` impl.
    // Ensure that each field implements `ReadOnlySystemParam`.
    let mut read_only_generics = generics.clone();
    let read_only_where_clause = read_only_generics.make_where_clause();
    for field_type in &field_types {
        read_only_where_clause
            .predicates
            .push(syn::parse_quote!(#field_type: #path::system::ReadOnlySystemParam));
    }
    let struct_name = &ast.ident;
    // let r = struct_name.to_string();
    let fields_alias =
        ensure_no_collision(format_ident!("{}__StructFieldsAlias", struct_name), token_stream.clone());
   
    // let state_struct_visibility = &ast.vis;
    let state_struct_name = ensure_no_collision(format_ident!("{}FetchState", struct_name), token_stream);

    TokenStream::from(quote! {
        impl<#punctuated_generics> #path::param_set::ParamSetElement for
            #struct_name <#(#shadowed_lifetimes,)* #punctuated_generic_idents> #where_clause
        {
            fn init_set_state<'w>(world: &'w #path::world::World, system_meta: &'w mut #path::system::SystemMeta) -> Self::State {
                #state_struct_name {
                    state: <#fields_alias::<'_, #punctuated_generic_idents> as #path::param_set::ParamSetElement>::init_set_state(world, system_meta),
                }
            }
        }
    })
}


#[proc_macro_derive(Bundle, attributes(bundle))]
pub fn derive_bundle(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let world_path = ecs_path();

    let named_fields = match get_named_struct_fields(&ast.data) {
        Ok(fields) => &fields.named,
        Err(e) => return e.into_compile_error().into(),
    };

    let field_types = named_fields
        .iter()
        .map(|field| &field.ty)
        .collect::<Vec<_>>();

    let idens = named_fields
        .iter()
        .map(|field| {let r = &field.ident; quote! { #r }})
        .collect::<Vec<_>>();

    let len = idens.len();
    let indexs = (0..len).into_iter()
        .map(|i| syn::Index::from(i) )
        .collect::<Vec<_>>();
 

    let tuple_types: Vec<_> = field_types.iter().map(|x| quote! { #x }).collect();
    let struct_name = &ast.ident;
    let generics = ast.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    
    TokenStream::from(quote! {
        const _: () = {
            impl #impl_generics #world_path::insert::Bundle for #struct_name #ty_generics #where_clause {
                type Item = Self;
                type State = (#(#world_path::insert::TState<#tuple_types>,)*);

                fn components() -> Vec<#world_path::archetype::ComponentInfo> {
                    vec![
                        #(
                            #world_path::archetype::ComponentInfo::of::<#tuple_types>(),
                        )*
                    ]
                }
                fn init_state(_world: & #world_path::world::World, _archetype: & #world_path::archetype::Archetype) -> Self::State {
                    (#(#world_path::insert::TState::new(_archetype.get_column(&std::any::TypeId::of::<#tuple_types>()).unwrap()),)*)
                }

                fn insert(
                    _state: &Self::State,
                    components: Self::Item,
                    _e: #world_path::world::Entity,
                    _row: #world_path::archetype::Row,
                ) {
                    #(
                        _state.#indexs.write(_e, _row, components.#idens);
                    )*

                }
            }
        };
    })
}

/// Finds an identifier that will not conflict with the specified set of tokens.
/// If the identifier is present in `haystack`, extra characters will be added
/// to it until it no longer conflicts with anything.
///
/// Note that the returned identifier can still conflict in niche cases,
/// such as if an identifier in `haystack` is hidden behind an un-expanded macro.
fn ensure_no_collision(value: Ident, haystack: TokenStream) -> Ident {
    // Collect all the identifiers in `haystack` into a set.
    let idents = {
        // List of token streams that will be visited in future loop iterations.
        let mut unvisited = vec![haystack];
        // Identifiers we have found while searching tokens.
        let mut found = FxHashSet::default();
        while let Some(tokens) = unvisited.pop() {
            for t in tokens {
                match t {
                    // Collect any identifiers we encounter.
                    TokenTree::Ident(ident) => {
                        found.insert(ident.to_string());
                    }
                    // Queue up nested token streams to be visited in a future loop iteration.
                    TokenTree::Group(g) => unvisited.push(g.stream()),
                    TokenTree::Punct(_) | TokenTree::Literal(_) => {}
                }
            }
        }

        found
    };

    let span = value.span();

    // If there's a collision, add more characters to the identifier
    // until it doesn't collide with anything anymore.
    let mut value = value.to_string();
    while idents.contains(&value) {
        value.push('X');
    }

    Ident::new(&value, span)
}


fn derive_label_inner(input: TokenStream, label: &str) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let mut trait_path = ecs_path();
    trait_path
    .segments
    .push(format_ident!("schedule_config").into());

    let mut dyn_eq_path = trait_path.clone();
    trait_path
        .segments
        .push(syn::Ident::new(label, proc_macro2::Span::call_site()).into());

    dyn_eq_path.segments.push(format_ident!("DynEq").into());

    derive_label(input, "StageLabel", &trait_path, &dyn_eq_path)
}

/// Get the fields of a data structure if that structure is a struct with named fields;
/// otherwise, return a compile error that points to the site of the macro invocation.
fn get_named_struct_fields(data: &syn::Data) -> syn::Result<&FieldsNamed> {
    match data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => Ok(fields),
        _ => Err(Error::new(
            // This deliberately points to the call site rather than the structure
            // body; marking the entire body as the source of the error makes it
            // impossible to figure out which `derive` has a problem.
            Span::call_site().into(),
            "Only structs with named fields are supported",
        )),
    }
}


pub(crate) fn ecs_path() -> syn::Path {
    let mut path = ECS_PATH.lock().unwrap();
    let path = match &*path {
        Some(r) => syn::parse_str(r).unwrap(),
        None => {
            let p = Manifest::default().get_path("pi_world");
            *path = Some(quote::quote!(#p).to_string());
            p
        },
    };
    path.clone()
}

// pub(crate) fn bevy_utils_path() -> syn::Path {
//     let mut path = BEVY_UTILS.lock().unwrap();
//     let path = match &*path {
//         Some(r) => syn::parse_str(r).unwrap(),
//         None => {
//             let p = Manifest::default().get_path("bevy_utils");
//             *path = Some(quote::quote!(#p).to_string());
//             p
//         },
//     };
//     path.clone()
// }

lazy_static! {
    static ref ECS_PATH: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    // static ref BEVY_UTILS: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
}

fn get_idents(fmt_string: fn(usize) -> String, count: usize) -> Vec<Ident> {
    (0..count)
        .map(|i| Ident::new(&fmt_string(i), proc_macro2::Span::call_site()))
        .collect::<Vec<Ident>>()
}

// use bevy_utils::label::DynEq
#[proc_macro]
pub fn impl_param_set(_input: TokenStream) -> TokenStream {
    let mut tokens = TokenStream::new();
    let max_params = 8;
    let params = get_idents(|i| format!("P{i}"), max_params);
    // let metas = get_idents(|i| format!("m{i}"), max_params);
    let mut param_fn_muts = Vec::new();
    for (i, param) in params.iter().enumerate() {
        let fn_name = Ident::new(&format!("p{i}"), proc_macro2::Span::call_site());
        let index = Index::from(i);
        let ordinal = match i {
            1 => "1st".to_owned(),
            2 => "2nd".to_owned(),
            3 => "3rd".to_owned(),
            x => format!("{x}th"),
        };
        let comment =
            format!("Gets exclusive access to the {ordinal} parameter in this [`ParamSet`].");
        param_fn_muts.push(quote! {
            #[doc = #comment]
            /// No other parameters may be accessed while this one is active.
            pub fn #fn_name(&mut self) -> &mut SystemParamItem<'w, #param>{
                // SAFETY: systems run without conflicts with other systems.
                // Conflicting params in ParamSet are not accessible at the same time
                // ParamSets are guaranteed to not conflict with other SystemParams
                &mut self.0.#index
            }
        });
    }

    for param_count in 1..=max_params {
        let param = &params[0..param_count];
        // let meta = &metas[0..param_count];
        let param_fn_mut = &param_fn_muts[0..param_count];
        tokens.extend(TokenStream::from(quote! {

            impl<'w,  #(#param: ParamSetElement + 'static,)*> ParamSet<'w, (#(#param,)*)>
            {
                #(#param_fn_mut)*
            }
        }));
    }

    tokens
}