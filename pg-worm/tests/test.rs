use pg_worm::Model;

struct Bar {
    id: i64,
}

impl TryFrom<::pg_worm::pg::Row> for Bar {
    type Error = ::pg_worm::Error;
    fn try_from(value: ::pg_worm::pg::Row) -> Result<Self, Self::Error> {
        Ok(Bar { id: value.try_get("_id")? })
    }
}

impl ::pg_worm::FromRow for Bar {}

impl Bar {
    const id: ::pg_worm::query::TypedColumn<i64> = ::pg_worm::query::TypedColumn::new(
        "Bar",
        "_id",
    );
}

impl Bar {
    const columns: [::pg_worm::query::Column; 1usize] = [Bar::id.column];
}

impl ::pg_worm::Model<Bar> for Bar {
    fn table() -> ::pg_worm::migration::Table {
        unimplemented!();
    }
    fn select<'a>() -> ::pg_worm::query::Select<'a, Vec<Bar>> {
        ::pg_worm::query::Select::new(&Bar::columns, "Bar")
    }
    fn select_one<'a>() -> ::pg_worm::query::Select<'a, Option<Bar>> {
        ::pg_worm::query::Select::new(&Bar::columns, "Bar")
    }
    fn update<'a>() -> ::pg_worm::query::Update<'a, ::pg_worm::query::NoneSet> {
        ::pg_worm::query::Update::<::pg_worm::query::NoneSet>::new("Bar")
    }
    fn delete<'a>() -> ::pg_worm::query::Delete<'a> {
        ::pg_worm::query::Delete::new("Bar")
    }
    fn query<'a>(
        query: impl Into<String>,
        params: Vec<&'a (dyn ::pg_worm::pg::types::ToSql + Sync)>,
    ) -> ::pg_worm::query::Query<'a, Vec<Bar>> {
        let query: String = query.into();
        ::pg_worm::query::Query::new(query, params)
    }
}

impl Bar {
    async fn foo() {
        Bar::query("", vec![]).await;
    }
}

#[tokio::main]
async fn main() -> () {
    todo!()
}
