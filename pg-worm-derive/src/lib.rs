mod parse;

use darling::FromDeriveInput;
use proc_macro::{self, TokenStream};
use syn::parse_macro_input;

use parse::ModelInput;

#[proc_macro_derive(Model, attributes(table, column))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = ModelInput::from_derive_input(&parse_macro_input!(input)).unwrap();

    let output = input.impl_everything();

    output.into()
}

#[cfg(test)]
mod tests {
    use darling::FromDeriveInput;
    use syn::parse_str;

    use crate::parse::ModelInput;

    #[test]
    fn test() {
        let input = r#"
            #[derive(Model)]
            struct Book {
                #[column(primary_key, auto)]
                id: i64,
                title: String
            }
        "#;
        let tokens = parse_str(input).unwrap();
        let parsed_input = ModelInput::from_derive_input(&tokens).unwrap();
        let _output = parsed_input.impl_everything();
    }
}
