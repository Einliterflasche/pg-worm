use proc_macro::{self, TokenStream};
use quote::{quote, format_ident};
use syn::{parse_macro_input, DeriveInput, Data};

#[proc_macro_derive(Entity)]
pub fn derive(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, data, attrs, .. } = parse_macro_input!(input);

    let data_struct = match data {
        Data::Struct(data_struct) => data_struct,
        _ => panic!("only structs")
    };

    let named_fields: Vec<_> = match data_struct.fields {
        syn::Fields::Named(named_fields) => named_fields,
        _ => panic!("no named fields")
    }.named.into_iter().map(|f| f.ident.unwrap()).collect();

    let strs = named_fields.iter().map(|f| format_ident!("{}", f).to_string());

    let output = quote! {
        impl Entity<#ident> for #ident {
            fn from_sql(row: &tokio_postgres::Row) -> Result<#ident, tokio_postgres::Error> {
                Ok(#ident {
                    #(#named_fields: row.try_get(#strs)?),*
                })
            }
        }
    };
    output.into()
}
