use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

/// Derive a label trait
///
/// # Args
///
/// - `input`: The [`syn::DeriveInput`] for struct that is deriving the label trait
/// - `trait_name`: Name of the label trait
/// - `trait_path`: The [path](`syn::Path`) to the label trait
/// - `dyn_eq_path`: The [path](`syn::Path`) to the `DynEq` trait
pub fn derive_label(
    input: syn::DeriveInput,
    trait_name: &str,
    trait_path: &syn::Path,
    dyn_eq_path: &syn::Path,
) -> TokenStream {
    if let syn::Data::Union(_) = &input.data {
        let message = format!("Cannot derive {trait_name} for unions.");
        return quote_spanned! {
            input.span() => compile_error!(#message);
        }
        .into();
    }

    let ident = input.ident.clone();
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mut where_clause = where_clause.cloned().unwrap_or_else(|| syn::WhereClause {
        where_token: Default::default(),
        predicates: Default::default(),
    });
    where_clause.predicates.push(
        syn::parse2(quote! {
            Self: 'static + Send + Sync + Clone + Eq + ::std::fmt::Debug + ::std::hash::Hash
        })
        .unwrap(),
    );
    quote! {
        impl #impl_generics #trait_path for #ident #ty_generics #where_clause {
            fn dyn_clone(&self) -> ::std::boxed::Box<dyn #trait_path> {
                ::std::boxed::Box::new(::std::clone::Clone::clone(self))
            }

            fn as_dyn_eq(&self) -> &dyn #dyn_eq_path {
                self
            }

            fn dyn_hash(&self, mut state: &mut dyn ::std::hash::Hasher) {
                let ty_id = ::std::any::TypeId::of::<Self>();
                ::std::hash::Hash::hash(&ty_id, &mut state);
                ::std::hash::Hash::hash(self, &mut state);
            }
        }
    }
    .into()
}
