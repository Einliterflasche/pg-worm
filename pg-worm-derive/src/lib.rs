use std::ops::{Deref, DerefMut};

use convert_case::{Case, Casing};
use proc_macro::{self, TokenStream as OldTokenStream};
use proc_macro2::{Ident, TokenStream};
use quote::spanned::Spanned;
use syn::{
    parse_macro_input, punctuated::Punctuated, Data, DeriveInput, Error, Expr, Fields, Lit, Meta,
};

#[proc_macro_derive(Model, attributes(table, column))]
pub fn derive(input: OldTokenStream) -> OldTokenStream {
    let parsed = parse_macro_input!(input as DeriveInput);

    let _model = match Model::try_from_input(parsed.clone()) {
        Ok(model) => model,
        Err(e) => return e.into_compile_error().into(),
    };

    let mut out = TokenStream::new();

    // Throw an error if generics are used
    if !parsed.generics.params.is_empty() {
        out.extend(
            Error::new_spanned(
                parsed.generics,
                "pg-worm: cannot derive `Model` for struct with generic parameters",
            )
            .to_compile_error(),
        );
    }

    out.extend(_model.errs.iter().map(Error::to_compile_error));

    out.into()
}

struct Skeleton<T> {
    val: T,
    errs: Vec<Error>,
}

struct Model {
    ident: Ident,
    table_name: String,
    primary_keys: Vec<String>,
    fields: Vec<Field>,
}

struct Field {
    ident: Ident,
    column_name: String,
    ty: syn::Type,
    primary_key: bool,
    auto_generate: bool,
}

impl Model {
    fn try_from_input(input: DeriveInput) -> Result<Skeleton<Model>, Error> {
        let mut errs: Vec<Error> = Vec::new();

        let Data::Struct(data_struct) = input.data.clone() else {
            return Err(Error::new(
                input.clone().__span(),
                "pg-worm: `Model` must be derived for struct with named fields",
            ));
        };

        let Fields::Named(named_fields) = data_struct.fields else {
            return Err(Error::new(
                input.clone().__span(),
                "pg-worm: `Model` must be derived for struct with named fields",
            ));
        };

        let fields = named_fields.named.into_iter().collect::<Vec<_>>();

        if fields.is_empty() {
            return Err(Error::new(
                input.ident.clone().span(),
                "pg-worm: cannot derive `Model` for struct without (named) fields",
            ));
        }

        let mut parsed_fields = Vec::new();

        for (field, field_errs) in fields.into_iter().map(Field::try_parse) {
            errs.extend(field_errs);
            parsed_fields.push(field);
        }

        Ok(Skeleton::new(
            Model {
                ident: input.ident.clone(),
                table_name: input.ident.to_string(),
                primary_keys: vec![],
                fields: parsed_fields,
            },
            errs,
        ))
    }
}

impl Field {
    fn try_parse(value: syn::Field) -> (Self, Vec<Error>) {
        let mut errs = Vec::new();

        let attr = value
            .attrs
            .into_iter()
            .find(|i| i.path().is_ident("column"));

        let mut column_name = value
            .ident
            .clone()
            .unwrap()
            .to_string()
            .to_case(Case::Snake);

        let mut primary_key = false;

        if let Some(attr) = attr {
            if let Ok(nested) =
                attr.parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)
            {
                for meta in nested {
                    match meta {
                        Meta::Path(path) if path.is_ident("primary_key") => {
                            primary_key = true;
                        }
                        Meta::NameValue(meta) if meta.path.is_ident("name") => match meta.value {
                            Expr::Lit(lit) => match lit.lit {
                                Lit::Str(str_lit) => column_name = str_lit.value(),
                                _ => errs.push(Error::new_spanned(
                                    lit,
                                    "pg-worm: option `name` must be string literal",
                                )),
                            },
                            _ => errs.push(Error::new_spanned(
                                meta,
                                "pg-worm: option `name` must be string literal",
                            )),
                        },
                        _ => errs.push(Error::new_spanned(meta, "pg-worm: unknown macro option")),
                    }
                }
            }
        }

        let field = Self {
            ident: value
                .ident
                .clone()
                .expect("pg-worm: field has no identifier, unnamed fields are not supported"),
            column_name,
            ty: value.ty,
            primary_key,
            auto_generate: false,
        };

        (field, errs)
    }
}

impl<T> Skeleton<T> {
    fn new(val: T, errs: impl IntoIterator<Item = Error>) -> Skeleton<T> {
        Self {
            val,
            errs: errs.into_iter().collect(),
        }
    }
}

impl<T> Deref for Skeleton<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.val
    }
}

impl<T> DerefMut for Skeleton<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.val
    }
}
