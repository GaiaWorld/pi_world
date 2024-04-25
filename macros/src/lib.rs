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

    let fields_alias =
        ensure_no_collision(format_ident!("__StructFieldsAlias"), token_stream.clone());

    let struct_name = &ast.ident;
    let state_struct_visibility = &ast.vis;
    let state_struct_name = ensure_no_collision(format_ident!("FetchState"), token_stream);

    TokenStream::from(quote! {
        // We define the FetchState struct in an anonymous scope to avoid polluting the user namespace.
        // The struct can still be accessed via SystemParam::State, e.g. EventReaderState can be accessed via
        // <EventReader<'static, 'static, T> as SystemParam>::State
        const _: () = {
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

                // fn new_archetype(state: &mut Self::State, archetype: &#path::archetype::Archetype, system_meta: &mut #path::system::SystemMeta) {
                //     <#fields_alias::<'_, '_, #punctuated_generic_idents> as #path::prelude::SystemParam>::new_archetype(&mut state, archetype, system_meta)
                // }

                // fn apply(state: &mut Self::State, system_meta: &#path::prelude::SystemMeta, world: &mut #path::world::World) {
                //     <#fields_alias::<'_, '_, #punctuated_generic_idents> as #path::prelude::SystemParam>::apply(&mut state, system_meta, world);
                // }

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
        };
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

    
    TokenStream::from(quote! {
        const _: () = {
            impl #world_path::insert::InsertComponents for #struct_name {
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

// use bevy_utils::label::DynEq