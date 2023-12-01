use std::ops::{Deref, DerefMut};

use convert_case::{Case, Casing};
use proc_macro::{self, TokenStream as OldTokenStream};
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{
    parse_macro_input, punctuated::Punctuated, Data, DeriveInput, Error, Expr, Fields, Lit, Meta,
};

#[proc_macro_derive(Model, attributes(table, column))]
pub fn derive(input: OldTokenStream) -> OldTokenStream {
    let parsed = parse_macro_input!(input as DeriveInput);

    let model = Model::try_from_input(parsed.clone());

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

    out.extend(model.errs.iter().map(Error::to_compile_error));
    out.extend(model.impl_from_row());

    out.extend(model.impl_column_consts());
    out.extend(model.impl_columns_array());
    out.extend(model.impl_model());

    out.into()
}

struct Skeleton<T> {
    pub val: T,
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
}

impl Model {
    fn try_from_input(input: DeriveInput) -> Skeleton<Model> {
        let mut errs: Vec<Error> = Vec::new();
        let mut fields: Vec<syn::Field> = Vec::new();

        match input.data.clone() {
            Data::Enum(_) | Data::Union(_) => errs.push(Error::new_spanned(
                &input,
                "pg-worm: `Model` must be derived for struct with named fields",
            )),
            Data::Struct(data_struct) => match data_struct.fields {
                Fields::Unnamed(_) | Fields::Unit => errs.push(Error::new_spanned(
                    data_struct.fields,
                    "pg-worm: `Model` must be derived for struct with named fields",
                )),
                Fields::Named(named_fields) => fields.extend(named_fields.named),
            },
        };

        let mut parsed_fields = Vec::new();

        if fields.is_empty() {
            errs.push(Error::new_spanned(
                &input,
                "pg-worm: `Model` must be derived for struct with at least one field",
            ));
        }

        for skeleton_field in fields.into_iter().map(Field::try_parse) {
            errs.extend(skeleton_field.errs);
            parsed_fields.push(skeleton_field.val);
        }

        let model = Model {
            ident: input.ident.clone(),
            table_name: input.ident.to_string(),
            primary_keys: vec![],
            fields: parsed_fields,
        };

        Skeleton::new(model, errs)
    }

    fn impl_column_consts(&self) -> TokenStream {
        if self.fields.is_empty() {
            return quote!();
        }

        let ident = &self.ident;
        let consts = self
            .fields
            .iter()
            .map(|i| i.impl_column_const(&self.table_name));

        quote!(
            #[automatically_derived]
            impl #ident {
                #(#consts)*
            }
        )
    }

    fn impl_from_row(&self) -> TokenStream {
        let field_idents = self.fields.iter().map(|i| &i.ident);
        let column_names = self.fields.iter().map(|i| &i.column_name);
        let ident = self.ident.clone();

        if self.fields.is_empty() {
            return quote!(
                #[automatically_derived]
                impl TryFrom<::pg_worm::pg::Row> for #ident {
                    type Error = ::pg_worm::Error;

                    fn try_from(_: ::pg_worm::pg::Row) -> Result<Self, Self::Error> {
                        unimplemented!()
                    }
                }

                #[automatically_derived]
                impl ::pg_worm::FromRow for #ident { }
            );
        }

        quote!(
            #[automatically_derived]
            impl TryFrom<::pg_worm::pg::Row> for #ident {
                type Error = ::pg_worm::Error;

                fn try_from(value: ::pg_worm::pg::Row) -> Result<Self, Self::Error> {
                    Ok(#ident {
                        #(
                            #field_idents: value.try_get(#column_names)?
                        ),*
                    })
                }
            }

            #[automatically_derived]
            impl ::pg_worm::FromRow for #ident { }
        )
    }

    fn impl_columns_array(&self) -> TokenStream {
        let ident = &self.ident;
        let field_idents = self.fields.iter().map(|i| &i.ident);
        let num_fields = self.fields.len();

        quote!(
            impl #ident {
                #[automatically_derived]
                const columns: [::pg_worm::query::Column; #num_fields] = [
                    #(
                        *#ident::#field_idents
                    ),*
                ];
            }
        )
    }

    fn impl_model(&self) -> TokenStream {
        let ident = &self.ident;
        let table_name = &self.table_name;

        quote!(
            #[automatically_derived]
            impl ::pg_worm::Model<#ident> for #ident {
                fn table() -> ::pg_worm::migration::Table {
                    unimplemented!();
                }

                fn select<'a>() -> ::pg_worm::query::Select<'a, Vec<#ident>> {
                    ::pg_worm::query::Select::new(&#ident::columns, #table_name)
                }

                fn select_one<'a>() -> ::pg_worm::query::Select<'a, Option<#ident>> {
                    ::pg_worm::query::Select::new(&#ident::columns, #table_name)
                }

                fn update<'a>() -> ::pg_worm::query::Update<'a> {
                    ::pg_worm::query::Update::new(#table_name)
                }


                fn delete<'a>() -> ::pg_worm::query::Delete<'a> {
                    ::pg_worm::query::Delete::new(#table_name)
                }

                fn query<'a>(query: impl Into<String>, params: Vec<&'a (dyn ::pg_worm::pg::types::ToSql + Sync)>)
                -> ::pg_worm::query::Query<'a, Vec<#ident>> {
                    let query: String = query.into();
                    ::pg_worm::query::Query::new(query, params)
                }
            }
        )
    }
}

impl Field {
    fn try_parse(value: syn::Field) -> Skeleton<Self> {
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
        };

        Skeleton::new(field, errs)
    }

    fn impl_column_const(&self, table_name: &str) -> TokenStream {
        let ty = &self.ty;
        let ident = &self.ident;

        let column_name = &self.column_name;

        quote!(
            #[automatically_derived]
            const #ident: ::pg_worm::query::TypedColumn<#ty> = ::pg_worm::query::TypedColumn::new(
                #table_name,
                #column_name
            );
        )
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
