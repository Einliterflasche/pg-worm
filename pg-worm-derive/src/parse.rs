use darling::{ast::Data, FromDeriveInput, FromField};
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
    dtype: Option<String>,
    column_name: Option<String>,
    #[darling(default)]
    auto: bool,
    #[darling(default)]
    primary_key: bool,
    #[darling(default)]
    unique: bool,
    #[darling(skip)]
    nullable: bool,
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

        self.ident.to_string().to_lowercase()
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
        self.all_fields().filter(|f| !(*f).auto)
    }

    /// Generate the SQL statement needed to create
    /// the table corresponding to the input.
    fn table_creation_sql(&self) -> String {
        format!(
            "CREATE TABLE {} ({})",
            self.table_name(),
            self.all_fields()
                .map(|f| f.column_creation_sql())
                .collect::<Vec<String>>()
                .join(", ")
        )
    }

    /// Generate all code needed.
    pub fn impl_everything(&self) -> TokenStream {
        let ident = self.ident();

        let try_from_row = self.impl_try_from_row();
        let column_consts = self.impl_column_consts();
        let columns = self.impl_columns();
        let insert = self.impl_insert();
        let model = self.impl_model();
        let to_model = self.impl_to_model();

        quote!(
             impl #ident {
                 #column_consts
                 #insert
                 #columns
             }


             #try_from_row
             #model
             #to_model
        )
    }

    fn impl_to_model(&self) -> TokenStream {
        let ident = self.ident();

        quote!(
            impl pg_worm::ToModel<#ident> for Vec<pg_worm::Row> {
                fn to_model(&self) -> Result<Vec<#ident>, pg_worm::Error> {
                    self
                        .iter()
                        .map(|i| #ident::try_from(i))
                        .collect()
                }
            }
        )
    }

    /// Generate the code for implementing the
    /// `Model` trait.
    fn impl_model(&self) -> TokenStream {
        let ident = self.ident();
        let table_name = self.table_name();
        let creation_sql = self.table_creation_sql();

        let select = self.impl_select();
        let select_one = self.impl_select_one();
        let delete = self.impl_delete();

        quote!(
            #[pg_worm::async_trait]
            impl pg_worm::Model<#ident> for #ident {
                #select
                #select_one
                #delete

                fn table_name() -> &'static str {
                    #table_name
                }

                fn _table_creation_sql() -> &'static str {
                    #creation_sql
                }

                fn columns() -> &'static [&'static pg_worm::DynCol] {
                    &#ident::COLUMNS
                }
            }
        )
    }

    /// Generate the code for
    /// the delete method.
    fn impl_delete(&self) -> TokenStream {
        let ident = self.ident();

        quote!(
            async fn delete(filter: pg_worm::Filter) -> u64 {
                use pg_worm::prelude::*;

                let query = QueryBuilder::<Delete>::new(#ident::COLUMNS)
                    .filter(filter)
                    .build();

                let res = query
                    .exec()
                    .await
                    .expect("couldn't make query");

                res
            }
        )
    }

    /// Generate the code for the
    /// select method.
    fn impl_select(&self) -> TokenStream {
        let ident = self.ident();

        quote!(
            async fn select(filter: pg_worm::Filter) -> Vec<#ident> {
                use pg_worm::prelude::*;

                let query = QueryBuilder::<Select>::new(#ident::COLUMNS)
                    .filter(filter)
                    .build();

                let res = query
                    .exec()
                    .await
                    .expect("couldn't make query")
                    .to_model()
                    .expect("couldn't parse response to struct");

                res
            }
        )
    }

    /// Generate the code for `select_one`
    fn impl_select_one(&self) -> TokenStream {
        let ident = self.ident();

        quote!(
            async fn select_one(filter: pg_worm::Filter) -> Option<#ident> {
                use pg_worm::prelude::*;

                let query = QueryBuilder::<Select>::new(#ident::COLUMNS)
                    .filter(filter)
                    .build();

                let res: Vec<#ident> = query
                    .exec()
                    .await
                    .expect("couldn't make query")
                    .to_model()
                    .expect("couldn't parse response to struct");

                res.into_iter().next()
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
            impl<'a> TryFrom<&'a pg_worm::Row> for #ident {
                type Error = pg_worm::Error;

                fn try_from(row: &'a pg_worm::Row) -> Result<#ident, Self::Error> {
                    let res = #ident {
                        #(
                            #field_idents: row.try_get(#column_names)?
                        ),*
                    };

                    Ok(res)
                }
            }
        )
    }

    /// Generate the code needed for
    /// creating the `COLUMNS` constant.
    fn impl_columns(&self) -> TokenStream {
        let ident = self.ident();
        let field_idents = self.all_fields().map(|i| i.ident());
        let n_fields = self.all_fields().count();

        quote!(
            pub const COLUMNS: [&'static pg_worm::DynCol; #n_fields] = [
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

        let column_counter = (1..=self.non_generated_fields().count())
            .map(|i| format!("${i}"))
            .collect::<Vec<_>>()
            .join(", ");

        let column_idents = self
            .non_generated_fields()
            .map(|f| f.ident())
            .collect::<Vec<_>>();
        let column_concrete_types = self.non_generated_fields().map(|f| f.ty.to_token_stream());
        let column_dtypes = self
            .non_generated_fields()
            .map(|f| f.insert_arg_type())
            .collect::<Vec<_>>();

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
                #(#column_idents: #column_dtypes),*
            ) -> Result<(), pg_worm::Error> {
                // Prepare sql statement
                let stmt = format!(
                    "INSERT INTO {} ({}) VALUES ({})",
                    #table_name,
                    #column_names,
                    #column_counter
                );

                // Convert to concrete types
                #(
                    let #column_idents: #column_concrete_types = #column_idents.into();
                ) *

                // Retrieve the client
                let client = pg_worm::_get_client()?;

                // Execute the query
                client.execute(
                    stmt.as_str(),
                    &[
                        #(&#column_idents),*
                    ]
                ).await?;

                // Everything's fine
                Ok(())
            }
        )
    }
}

impl ModelField {
    /// Initialization function called before each
    /// field is stored.
    fn init(mut field: ModelField) -> darling::Result<ModelField> {
        let ty = &field.ty;

        // Extract relevant type from the path
        let syn::Type::Path(path) = ty else {
            panic!("field type must be valid path");
        };
        let path = &path.path;
        let last_seg = path.segments.last().expect("must provide type");

        // If it's an Option<T>, set the field nullable
        if last_seg.ident.to_string() == "Option".to_string() {
            field.nullable = true;
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

    /// Get the corresponding column's PostgreSQL datatype.
    fn pg_datatype(&self) -> Type {
        if let Some(dtype) = &self.dtype {
            let ty = match dtype.to_lowercase().as_str() {
                "bool" | "boolean" => Type::BOOL,
                "text" => Type::TEXT,
                "int" | "integer" | "int4" => Type::INT4,
                "bigint" | "int8" => Type::INT8,
                "smallint" | "int2" => Type::INT2,
                "real" => Type::FLOAT4,
                "double precision" => Type::FLOAT8,
                "bigserial" => Type::INT8,
                _ => panic!("couldn't find postgres type `{}`", dtype),
            };

            return ty;
        }

        match &self.ty {
            syn::Type::Path(type_path) => {
                let segment = type_path.path.segments.last().unwrap();
                // Support Option<T> as nullable T
                if segment.ident.to_string().as_str() == "Option" {
                    let args = &segment.arguments;
                    let PathArguments::AngleBracketed(angle_args) = args else {
                        panic!("weird option. should have angle brackets")
                    };
                }

                match type_path
                    .path
                    .segments
                    .last().unwrap()
                    .ident.to_string().as_str() {
                        "String" => Type::TEXT,
                        "i32" => Type::INT4,
                        "i64" => Type::INT8,
                        "f32" => Type::FLOAT4,
                        "f64" => Type::FLOAT8,
                        "bool" => Type::BOOL,
                        _ => todo!(
                            "cannot guess postgres type for field {:?}, please provide via attribute: `#[column(dtype = '<DataType>']`", 
                            self.ident().to_string()
                        )
                    }
                },
            syn::Type::Reference(_) => panic!("field {:?} may not be reference", self.ident().to_string()),
            _ => todo!(
                "cannot guess postgres type for field {:?}, please provide via attribute: `#[column(dtype = 'DataType']`", 
                self.ident().to_string()
            )
        }
    }

    /// Get the SQL representing the column needed
    /// for creating a table.
    fn column_creation_sql(&self) -> String {
        // The list of "args" for the sql statement.
        // Includes at least the column name and datatype.
        let mut args = vec![self.column_name(), self.pg_datatype().to_string()];

        // This macro allows adding an arg to the list
        // under a given condition.
        macro_rules! arg {
            ($cond:expr, $sql:literal) => {
                if $cond {
                    args.push($sql.to_string());
                }
            };
        }

        // Add possible args
        arg!(self.primary_key, "PRIMARY KEY");
        arg!(self.auto, "GENERATED ALWAYS AS IDENTITY");
        arg!(self.unique, "UNIQUE");

        // Join the args, seperated by a space and return them
        args.join(" ")
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
            pub const #ident: pg_worm::TypedColumn<#rs_type> = pg_worm::TypedColumn::new(#table_name, #col_name)
                #(#props)*;
        )
    }
}
