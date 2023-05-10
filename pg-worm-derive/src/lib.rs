use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data};

#[proc_macro_derive(Model)]
pub fn derive(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, data, .. } = parse_macro_input!(input);

    let field_idents = match data {
        Data::Struct(data_struct) => match data_struct.fields {
            syn::Fields::Named(fields) => 
                fields
                    .named
                    .into_iter()
                    .map(|f| f.ident.unwrap()),
            _ => panic!("need named fields")
        },
        _ => unimplemented!("only structs supported")
    };

    let output = quote! {
        impl Model<#ident> for #ident {
            fn from_row(row: &tokio_postgres::Row) -> Result<#ident, tokio_postgres::Error> {
                Ok(#ident {
                    #(#field_idents: row.try_get(stringify!(#field_idents))?),*
                })
            }
        }
    };
    output.into()
}
