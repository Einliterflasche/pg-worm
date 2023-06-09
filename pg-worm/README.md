![GitHub Actions Testing](https://github.com/Einliterflasche/pg-worm/actions/workflows/rust.yml/badge.svg)

# `pg-worm`
### *P*ost*g*reSQL's *W*orst *ORM*
`pg-worm` is a straightforward, fully typed, async ORM and Query Builder for PostgreSQL.
Well, at least that's the goal.

This library is based on [`tokio_postgres`](https://docs.rs/tokio-postgres/0.7.8/tokio_postgres/index.html) 
and is intended to be used with [`tokio`](https://tokio.rs/).

## Usage
Fortunately, using this library is very easy.

Just derive the `Model` trait for your type, connect to your database 
and you are ready to go!

Here's a quick example: 

```rust
use pg_worm::prelude::*;
use tokio::try_join;

#[derive(Model)]
struct Book {
    // An auto-generated primary key
    #[column(primary_key, auto)]
    id: i64,
    #[column(unique)]
    title: String,
    sub_title: Option<String>,
    pages: Vec<String>,
    author_id: i64
}

#[derive(Model)]
struct Author {
    #[column(primary_key, auto)]
    id: i64,
    name: String
}

#[tokio::main]
async fn main() -> Result<(), pg_worm::Error> {
    // First create a connection. This can be only done once.
    connect!("postgres://me:me@localhost:5432", NoTls).await?;

    // Then, register the model with the pg_worm client.
    // Use `register!` if you want to fail if a
    // table with the same name already exists.
    //
    // `force_register` drops the old table,
    // which is useful for development.
    //
    // If your tables already exist, skip this part.
    force_register!(Author, Book)?;

    // Next, insert some data.
    // This works by passing values for all
    // fields which aren't autogenerated.
    try_join!(
        Author::insert("Stephen King"),
        Author::insert("Martin Luther King"),
        Author::insert("Karl Marx"),
        Book::insert("Foo - Part I", "Subtitle".to_string(), vec!["Page 1".to_string()], 1),
        Book::insert("Foo - Part II", None, vec![], 2),
        Book::insert("Foo - Part III", None, vec![], 3)
    )?;

    // Let's start with a simple query for all books.
    let books: Vec<Book> = Book::select(Filter::all()).await;
    assert_eq!(books.len(), 3);

    // Or search for a specific book
    let book = Book::select_one(Book::title.like("Foo%II")).await;
    assert!(book.is_some());
    assert!(book.unwrap().sub_title.is_none());

    // Or make more complex queries using the query builder:

    // Select all books written by an author named `King`
    let king_books: Vec<Book> = QueryBuilder::<Select>::new(Book::COLUMNS)
        .filter(Author::name.like("%King%")) // Matches all names which include `King`
        .join(&Book::author_id, &Author::id, JoinType::Inner)
        .build()
        .exec().await?
        .to_model()?;
    assert_eq!(king_books.len(), 2);

    // Select all books with at least one pages.
    let books_with_pages: Vec<Book> = QueryBuilder::<Select>::new(Book::COLUMNS)
        .filter(!Book::pages.empty())
        .build()
        .exec()
        .await?
        .to_model()?;
    assert_eq!(books_with_pages.len(), 1);

    // Select all books without a subtitle.
    let books_without_sub: Vec<Book> = QueryBuilder::<Select>::new(Book::COLUMNS)
        .filter(Book::sub_title.null())
        .build()
        .exec()
        .await?
        .to_model()?;
    assert_eq!(books_without_sub.len(), 2);

    // Or delete a book, you don't like
    Book::delete(Book::title.eq("Foo - Part II")).await;

    Ok(())
}
```

## Filters
Filters can be used to easily include `WHERE` clauses in your queries. 

They can be constructed by calling functions of the respective column. 
`pg_worm` automatically constructs a `Column` constant for each field 
of your `Model`. 

A practical example would look like this:

```rust
MyModel::select(MyModel::my_field.eq(5))
```

Currently the following filter functions are supported:

 * `Filter::all()` - doesn't check anything
 * `T.eq(T)` - checks whether the column is equal to a given value
 * `T.one_of(Vec<T>)` - checks whether the column is (at least) one of the given values
 * `String.like(String)` - check whether a `String` is `LIKE` a given [pattern](https://www.postgresql.org/docs/current/functions-matching.html)
 * `Option<T>.null()` - check whether an optional column `IS NULL`
 * `Vec<T>.empty()` - check whether an array is empty
 * `Vec<T>.contains(T)` - check whether an array contains a given value
 
You can also do filter logic using `!`, `&` and `|`: `MyModel::id.eq(5) & !MyModel::optional_field.null())`.
This works as you expect logical OR, AND and NOT to work.
Please notice that, at this point, custom priorization via parantheses 
is **not possible**.

## Query Builder
Simply attaching a `Filter` to your query often does not suffice. 
For this reason, `pg-worm` provides a `QueryBuilder` interface for
constructing more complex queries. 

Start building your query by calling `Query::select()` and passing 
the columns you want to select. 
Normally you want to query all columns of a `Model` which you can do by passing 
`YourModel::columns()`.

You can modify your query using the following methods:

 * `.filter()` - add a `WHERE` clause
 * `.join()` - add a `JOIN` for querying accross tables/models
 * `.limit()` - add a `LIMIT` to how many rows are returned

After you have configured your query, build it using the `.build()` method.
Then, execute it by calling `.exec::<M>()`, where `M` is the `Model` which
should be parsed from the query result. It may be inferred.

## Opiniatedness
As mentioned before, `pg_worm` is opiniated in a number of ways. 
These include:

 * `panic`s. For the sake of convenience `pg_worm` only returns a  `Result` when 
   inserting data, since in that case Postgres might reject the data because of
   some constraint. 

   This means that should something go wrong, like:
    - the connection to the database collapsed,
    - `pg_worm` is unable to parse Postgres' response,
    - ...
   
   the program will panic.
 * ease of use. The goal of `pg_worm` is **not** to become an enterprise solution.
   If adding an option means infringing the ease of use then it will likely
   not be added.

## License
This project is dual-licensed under the MIT and Apache 2.0 licenses.
