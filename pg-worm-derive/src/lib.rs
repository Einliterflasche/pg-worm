use darling::{FromDeriveInput, ast::Data, FromField};
use proc_macro::{self, TokenStream};
use quote::quote;
use syn::parse_macro_input;

#[derive(Clone, FromField)]
#[darling(attributes(column))]
struct ModelField {
    ident: Option<syn::Ident>,
    #[darling(default)]
    unique: bool,
    #[darling(default)]
    nullable: bool,
    dtype: String
}

#[derive(FromDeriveInput)]
#[darling(
    attributes(table),
    supports(struct_named)
)]
struct ModelInput {
    ident: syn::Ident,
    data: Data<(), ModelField>,
    table_name: Option<String>
}

#[proc_macro_derive(Model, attributes(table, column))]
pub fn derive(input: TokenStream) -> TokenStream {
    let opts = ModelInput::from_derive_input(&parse_macro_input!(input)).unwrap();

    let ident = &opts.ident;

    // The table name is either the provided or
    // the snakecased type name
    let table_name = match opts.table_name {
        Some(table_name) => table_name,
        None => stringify!(&opts.ident).to_lowercase()
    };

    // Retrieve the struct's fields
    let fields = match opts.data {
        Data::Struct(fields) => fields.fields,
        _ => panic!("enums not supported")
    };

    // Get the fields' idents
    let field_idents = fields
        .clone()
        .into_iter()
        .map(|f| f.ident.unwrap());

    // Generate the needed impl code
    let output = quote!(
        impl Model<#ident> for #ident {
            fn from_row(row: &pg_worm::Row) -> Result<#ident, pg_worm::tokio_postgres::Error> {
                let client = pg_worm::get_client();
                // Parse each column into the corresponding field
                Ok(#ident {
                    #(#field_idents: row.try_get(stringify!(#field_idents))?),*
                })
            }
        }
    );

    output.into()
}
