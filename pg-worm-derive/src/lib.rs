mod parse;

use darling::FromDeriveInput;
use proc_macro::{self, TokenStream};
use quote::quote;
use syn::parse_macro_input;

use parse::ModelInput;

/// Automatically implement `Model` for your struct.
/// 
/// ## Attributes
///  * `table` - for structs:
///      - `table_name`: String, optional
///  * `column` - for struct fields:
///      - `dtype`: String, required,
///      - `unique`: bool, optional, default: `false`
#[proc_macro_derive(Model, attributes(table, column))]
pub fn derive(input: TokenStream) -> TokenStream {
    let opts = ModelInput::from_derive_input(&parse_macro_input!(input)).unwrap();

    let ident = opts.ident();

    let table_name = opts.table_name();

    // Retrieve the struct's fields
    let fields = opts.fields();

    // Get the fields' idents
    let field_idents = fields
        .map(|f| f.clone().ident());

    let create_sql = opts.get_create_sql();

    // Generate the needed impl code
    let output = quote!(
        #[pg_worm::async_trait]
        impl Model<#ident> for #ident {
            fn from_row(row: &pg_worm::Row) -> Result<#ident, pg_worm::tokio_postgres::Error> {
                // Parse each column into the corresponding field
                Ok(#ident {
                    #(#field_idents: row.try_get(stringify!(#field_idents))?),*
                })
            }

            fn create_sql() -> String {
                #create_sql.to_string()
            }

            /// Panics if `connect` has not been executed or failed.
            async fn select() -> Vec<#ident> {
                let client = pg_worm::get_client().expect("not connected to db");
                let rows = client.query(format!("SELECT * FROM {}", #table_name).as_str(), &[]).await.unwrap();
                rows.iter().map(|r| #ident::from_row(r).expect("couldn't parse data")).collect()
            }

            async fn select_one() -> Option<#ident> {
                let client = pg_worm::get_client().expect("not connected to db");
                let rows = client.query(format!("SELECT * FROM {} LIMIT 1", #table_name).as_str(), &[]).await.unwrap();
                if rows.len() != 1 {
                    return None;
                }
                Some(#ident::from_row(&rows[0]).unwrap())
            }
        }
    );

    output.into()
}
