use convert_case::Casing;
use darling::{ast::Data, Error, FromDeriveInput, FromField};
use postgres_types::Type;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{Ident, PathArguments};

#[derive(FromDeriveInput)]
#[darling(attributes(table), supports(struct_named))]
pub struct ModelInput {
    ident: syn::Ident,
    #[darling(map = ModelField::init)]
    data: Data<(), ModelField>,
    table_name: Option<String>,
}

#[derive(Clone, FromField)]
#[darling(attributes(column), and_then = "ModelField::init")]
pub struct ModelField {
    ident: Option<syn::Ident>,
    ty: syn::Type,
    column_name: Option<String>,
    #[darling(default)]
    auto: bool,
    #[darling(skip)]
    auto_gen_stmt: String,
    #[darling(default)]
    primary_key: bool,
    #[darling(default)]
    unique: bool,
    #[darling(skip)]
    nullable: bool,
    #[darling(skip)]
    array: bool,
}

impl ModelInput {
    /// Get the input's ident.
    const fn ident(&self) -> &Ident {
        &self.ident
    }

    /// Generate the table's name.
    fn table_name(&self) -> String {
        if let Some(table_name) = &self.table_name {
            return table_name.clone();
        }

        self.ident.to_string().to_case(convert_case::Case::Snake)
    }

    /// Get an iterator over the input struct's fields.
    fn all_fields(&self) -> impl Iterator<Item = &ModelField> {
        let Data::Struct(fields) = &self.data else {
            panic!("only named structs allowed");
        };

        fields.iter()
    }

    /// Get an iterator over the input struct's fields
    /// but skip the auto generated ones.
    fn non_generated_fields(&self) -> impl Iterator<Item = &ModelField> {
        self.all_fields().filter(|f| !f.auto)
    }

    /// Generate the SQL statement needed to create
    /// the table corresponding to the input.
    fn table_creation_sql(&self) -> Result<String, Error> {
        Ok(format!(
            "CREATE TABLE {} ({})",
            self.table_name(),
            self.all_fields()
                .map(|f| f.column_creation_sql())
                .collect::<Result<Vec<String>, Error>>()?
                .join(", ")
        ))
    }

    /// Generate all code needed.
    pub fn impl_everything(&self) -> TokenStream {
        let ident = self.ident();

        let try_from_row = self.impl_try_from_row();
        let column_consts = self.impl_column_consts();
        let columns = self.impl_columns();
        let insert = self.impl_insert();
        let model = self.impl_model();

        quote!(
            impl #ident {
                #column_consts
                #insert
                #columns
            }

            #try_from_row
            #model
        )
    }

    /// Generate the code for implementing the
    /// `Model` trait.
    fn impl_model(&self) -> TokenStream {
        let ident = self.ident();
        let table_name = self.table_name();
        let creation_sql = match self.table_creation_sql() {
            Ok(res) => quote!(#res),
            Err(err) => err.write_errors(),
        };

        let select = self.impl_select();
        let delete = self.impl_delete();
        let update = self.impl_update();
        let query = self.impl_query();

        quote!(
            #[pg_worm::async_trait]
            impl pg_worm::Model<#ident> for #ident {
                #select
                #update
                #delete
                #query

                fn table_name() -> &'static str {
                    #table_name
                }

                fn _table_creation_sql() -> &'static str {
                    #creation_sql
                }

                fn columns() -> &'static [&'static dyn Deref<Target = Column>] {
                    &#ident::COLUMNS
                }
            }
        )
    }

    fn impl_query(&self) -> TokenStream {
        let ident = self.ident();

        quote!(
            fn query<'a>(statement: impl Into<String>, params: Vec<&'a (dyn pg_worm::pg::types::ToSql + Sync)>) -> pg_worm::query::Query<'a, Vec<#ident>> {
                pg_worm::query::Query::new(
                    statement.into(),
                    params
                )
            }
        )
    }

    fn impl_update(&self) -> TokenStream {
        let ident = self.ident();

        quote!(
            fn update<'a>() -> pg_worm::query::Update<'a, NoneSet> {
                pg_worm::query::Update::<NoneSet>::new(#ident::table_name())
            }
        )
    }

    fn impl_delete(&self) -> TokenStream {
        let ident = self.ident();

        quote!(
            fn delete<'a>() -> pg_worm::query::Delete<'a> {
                pg_worm::query::Delete::new(#ident::table_name())
            }
        )
    }

    /// Generate the code for the
    /// select method.
    fn impl_select(&self) -> TokenStream {
        let ident = self.ident();

        quote!(
            fn select<'a>() -> pg_worm::query::Select<'a, Vec<#ident>> {
                pg_worm::query::Select::new(#ident::columns(), #ident::table_name())
            }

            fn select_one<'a>() -> pg_worm::query::Select<'a, Option<#ident>> {
                pg_worm::query::Select::new(#ident::columns(), #ident::table_name())
                    .limit(1)
            }
        )
    }

    /// Generate the code for implementing
    /// `TryFrom<&Row>`
    fn impl_try_from_row(&self) -> TokenStream {
        let ident = self.ident();
        let field_idents = self.all_fields().map(|i| i.ident());
        let column_names = self.all_fields().map(|i| i.column_name());

        quote!(
            impl TryFrom<pg_worm::pg::Row> for #ident {
                type Error = pg_worm::Error;

                fn try_from(row: pg_worm::pg::Row) -> Result<#ident, Self::Error> {
                    let res = #ident {
                        #(
                            #field_idents: row.try_get(#column_names)?
                        ),*
                    };

                    Ok(res)
                }
            }

            impl FromRow for #ident { }
        )
    }

    /// Generate the code needed for
    /// creating the `COLUMNS` constant.
    fn impl_columns(&self) -> TokenStream {
        let ident = self.ident();
        let field_idents = self.all_fields().map(|i| i.ident());
        let n_fields = self.all_fields().count();

        quote!(
            pub const COLUMNS: [&'static dyn Deref<Target = pg_worm::query::Column>; #n_fields] = [
                #(
                    &#ident::#field_idents
                ),*
            ];
        )
    }

    /// Generate the code for implementing
    /// the column constants.
    /// Needs to be wrapped in an `impl` block.
    fn impl_column_consts(&self) -> TokenStream {
        let column_consts = self.all_fields().map(|f| f.impl_column_const(self));
        quote!(
            #(#column_consts) *
        )
    }

    /// Generate the code for implementing
    /// the `insert` function.
    fn impl_insert(&self) -> TokenStream {
        let table_name = self.table_name();

        let column_names = self
            .non_generated_fields()
            .map(|f| f.column_name())
            .collect::<Vec<_>>()
            .join(", ");

        let placeholders = (1..=self.non_generated_fields().count())
            .map(|i| format!("${i}"))
            .collect::<Vec<_>>()
            .join(", ");

        let field_idents = self
            .non_generated_fields()
            .map(|f| f.ident())
            .collect::<Vec<_>>();

        let field_concrete_types = self.non_generated_fields().map(|f| f.ty.to_token_stream());
        let field_generic_types = self.non_generated_fields().map(|f| f.insert_arg_type());

        quote!(
            /// Insert a new entity into the database.
            ///
            /// For columns which are autogenerated (like in the example below, `id`),
            /// no value has to be specified.
            ///
            /// # Example
            ///
            /// ```ignore
            /// use pg_worm::Model;
            ///
            /// #[derive(Model)]
            /// struct Book {
            ///     #[column(primary_key, auto)]
            ///     id: i64,
            ///     title: String
            /// }
            ///
            /// async fn some_func() -> Result<(), pg_worm::Error> {
            ///     Book::insert("Foo".to_string()).await?;
            /// }
            /// ```
            pub async fn insert(
                #(#field_idents: #field_generic_types),*
            ) -> Result<(), pg_worm::Error> {
                // Format sql statement
                let stmt = format!(
                    "INSERT INTO {} ({}) VALUES ({})",
                    #table_name,
                    #column_names,
                    #placeholders
                );

                // Convert to concrete types
                #(
                    let #field_idents: #field_concrete_types = #field_idents.into();
                ) *

                // Retrieve the client
                let client = pg_worm::fetch_client().await?;

                // Execute the query
                client.execute(
                    stmt.as_str(),
                    &[
                        #(&#field_idents),*
                    ]
                ).await?;

                // Everything's fine
                Ok(())
            }
        )
    }
}

macro_rules! spanned_error {
    ($msg:expr, $err:expr) => {
        return Err(darling::Error::custom($msg).with_span($err))
    };
}

impl ModelField {
    /// Initialization function called before each
    /// field is stored.
    fn init(mut field: ModelField) -> darling::Result<ModelField> {
        let ty = &field.ty;

        // Extract relevant type from the path
        let syn::Type::Path(path) = ty else {
            spanned_error!("pg-worm: unsupported type", &ty)
        };
        let path = &path.path;
        let Some(last_seg) = path.segments.last() else {
            spanned_error!(
                "pg-worm: invalid type path (needs at least one segment)",
                &ty
            )
        };

        match last_seg.ident.to_string().as_str() {
            // If it's an Option<T>, set the field nullable
            "Option" => {
                field.nullable = true;
            }
            // If it's a Vec<T>, set the field to be an array
            "Vec" => {
                field.array = true;
            }
            _ => (),
        };

        if field.auto {
            field.auto_gen_stmt = match last_seg.ident.to_string().as_str() {
                #[cfg(feature = "uuid")]
                "Uuid" => "DEFAULT gen_random_uuid()",
                "i16" | "i32" | "i64" => "GENERATED ALWAYS AS IDENTITY",
                _ => spanned_error!("pg-worm: `auto` is only available for integers and uuid (with the `uuid` feature enabled)", &ty)
            }
            .to_string();
        }

        Ok(field)
    }

    /// Get the field's identifier.
    fn ident(&self) -> Ident {
        self.ident
            .clone()
            .expect("struct {} should only contain named fields")
    }

    /// Generate the column's name.
    fn column_name(&self) -> String {
        if let Some(column_name) = &self.column_name {
            return column_name.clone();
        }

        self.ident().to_string().to_lowercase()
    }

    /// Get the corresponding postgres type
    fn try_pg_datatype(&self) -> Result<Type, Error> {
        let ty = self.ty.clone();

        let syn::Type::Path(path) = &self.ty else {
            spanned_error!("pg-worm: unsupported type, must be a TypePath", &ty)
        };

        let Some(segment) = path.path.segments.last() else {
            spanned_error!(
                "pg-worm: unsupported type path, must have at least one segment",
                &ty
            )
        };

        let mut id = &segment.ident;

        if self.array || self.nullable {
            let PathArguments::AngleBracketed(args) = &segment.arguments else {
                spanned_error!(
                    "pg-worm: unsupported type, Option/Vec need generic argument",
                    &ty
                )
            };

            let Some(arg) = args.args.first() else {
                spanned_error!(
                    "pg-worm: unsupported type, Option/Vec need generic argument",
                    &ty
                )
            };

            let syn::GenericArgument::Type(arg_type) = arg else {
                spanned_error!(
                    "pg-worm: unsupported Option/Vec generic argument, must be valid type",
                    &ty
                )
            };

            let syn::Type::Path(path) = &arg_type else {
                spanned_error!("pg-worm: unsupported type, must be a TypePath", &ty)
            };

            let Some(segment) = path.path.segments.last() else {
                spanned_error!(
                    "pg-worm: unsupported type path, must have at least one segment",
                    &ty
                )
            };

            id = &segment.ident;
        }

        Ok(match id.to_string().as_ref() {
            "String" => Type::TEXT,
            "i16" => Type::INT2,
            "i32" => Type::INT4,
            "i64" => Type::INT8,
            "u64" => Type::INT8,
            "f32" => Type::FLOAT4,
            "f64" => Type::FLOAT8,
            "bool" => Type::BOOL,
            // `serde_json` support
            #[cfg(feature = "serde-json")]
            "Value" => Type::JSONB,
            // `time` support
            #[cfg(feature = "time")]
            "Date" => Type::DATE,
            #[cfg(feature = "time")]
            "Time" => Type::TIME,
            #[cfg(feature = "time")]
            "PrimitiveDateTime" => Type::TIMESTAMP,
            #[cfg(feature = "time")]
            "OffsetDateTime" => Type::TIMESTAMPTZ,
            // `uuid` support
            #[cfg(feature = "uuid")]
            "Uuid" => Type::UUID,
            _ => spanned_error!(
                "pg-worm: unsupported type. did you forget to enable a feature?",
                &ty
            ),
        })
    }

    /// Get the SQL representing the column needed
    /// for creating a table.
    fn column_creation_sql(&self) -> Result<String, Error> {
        // The list of "args" for the sql statement.
        // Includes at least the column name and datatype.
        let mut args = vec![self.column_name(), self.try_pg_datatype()?.to_string()];

        // This macro allows adding an arg to the list
        // under a given condition.
        macro_rules! arg {
            ($cond:expr, $sql:expr) => {
                if $cond {
                    args.push($sql.to_string());
                }
            };
        }

        // Add possible args
        arg!(self.array, "[]");
        arg!(self.primary_key, "PRIMARY KEY");
        arg!(self.auto, self.auto_gen_stmt);
        arg!(self.unique, "UNIQUE");
        arg!(!(self.primary_key || self.nullable), "NOT NULL");

        // Join the args, seperated by a space and return them
        Ok(args.join(" "))
    }

    /// The datatype which should be provided when
    /// calling the `insert` function.
    fn insert_arg_type(&self) -> TokenStream {
        let ty = self.ty.to_token_stream();
        quote!(impl Into<#ty> + pg_worm::pg::types::ToSql + Sync)
    }

    /// Generate the code for creating this field's
    /// column constant.
    fn impl_column_const(&self, table: &ModelInput) -> TokenStream {
        let table_name = table.table_name();
        let col_name = self.column_name();
        let ident = self.ident();
        let rs_type = &self.ty;

        // Vec containing the method calls.
        let mut props = Vec::new();

        // Macro for calling a method on the constant
        // under a given condition.
        macro_rules! prop {
            ($cond:expr, $id:ident) => {
                if $cond {
                    props.push(quote!(.$id()));
                }
            };
        }

        // Add the calls if needed
        prop!(self.auto, generated);
        prop!(self.unique, unique);
        prop!(self.primary_key, primary_key);
        prop!(self.nullable, nullable);

        quote!(
            #[allow(non_upper_case_globals)]
            pub const #ident: pg_worm::query::TypedColumn<#rs_type> = pg_worm::query::TypedColumn::new(#table_name, #col_name)
                #(#props)*;
        )
    }
}
