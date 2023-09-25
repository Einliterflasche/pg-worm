#![allow(dead_code)]

use pg_worm::{prelude::*, query::Prepared};

#[derive(Model)]
struct Book {
    #[column(primary_key, auto)]
    id: i64,
    title: String,
    sub_title: Option<String>,
    pages: Vec<String>,
    author_id: i64,
}

#[derive(Model)]
struct Author {
    #[column(primary_key, auto)]
    id: i64,
    name: String,
}

#[tokio::test]
async fn complete_procedure() -> Result<(), pg_worm::Error> {
    // First create a connection. This can be only done once.
    Connection::build("postgres://postgres:postgres@localhost:5432")
        .max_pool_size(16)
        .connect()
        .await?;
    println!("Hello World!");

    // Then, create the tables for your models.
    // Use `register!` if you want to fail if a
    // table with the same name already exists.
    //
    // `force_register` drops the old table,
    // which is useful for development.
    //
    // If your tables already exist, skip this part.
    force_create_table!(Author, Book).await?;

    // Next, insert some data.
    // This works by passing values for all
    // fields which aren't autogenerated.
    Author::insert("Stephen King").await?;
    Author::insert("Martin Luther King").await?;
    Author::insert("Karl Marx").await?;
    Book::insert(
        "Foo - Part I",
        "Subtitle".to_string(),
        vec!["Page 1".to_string()],
        1,
    )
    .await?;
    Book::insert("Foo - Part II", None, vec![], 2).await?;
    Book::insert("Foo - Part III", None, vec![], 3).await?;

    // Easily query for all books
    let books = Book::select().await?;
    assert_eq!(books.len(), 3);

    // Or check whether your favorite book is listed,
    // along some other arbitrary conditions
    let manifesto = Book::select_one()
        .where_(Book::title.eq(&"The Communist Manifesto".into()))
        .where_(Book::pages.contains(&"You have nothing to lose but your chains!".into()))
        .where_(Book::id.gt(&3))
        .prepared()
        .await?;
    assert!(manifesto.is_none());

    // Or update your records
    let books_updated = Book::update()
        .set(Book::title, &"The name of this book is a secret".into())
        .await?;
    assert_eq!(books_updated, 3);

    // Or run a raw query which gets automagically parsed to `Vec<Book>`.
    //
    // NOTE: You have to pass the exact type that Postgres is
    // expecting. Doing otherwise will result in a runtime error.
    let king_books = Book::query(
        r#"
            SELECT * FROM book 
            JOIN author ON author.id = book.author_id
            WHERE POSITION(? in author.name) > 0 
        "#,
        vec![&"King".to_string()],
    )
    .await?;
    assert_eq!(king_books.len(), 2);

    // Or do some array operations
    let page_1 = "Page 1".to_string();
    let page_2 = "Page 2".to_string();
    let pages = vec![&page_1, &page_2];

    let any_page = Book::select_one()
        .where_(Book::pages.contains_any(&pages))
        .await?;
    assert!(any_page.is_some());

    let both_pages = Book::select_one()
        .where_(Book::pages.contains_all(&pages))
        .await?;
    assert!(both_pages.is_none());

    // You can even do transactions:
    let transaction = Transaction::begin().await?;
    // Delete all books (in the transaction)
    transaction.execute(Book::delete()).await?;
    // Check that no books are left (in the transaction)
    let all_books_in_tx = transaction.execute(Book::select()).await?;
    assert_eq!(all_books_in_tx.len(), 0);

    // Verify that they still exist *outside* the transaction:
    let all_books_outside_tx = Book::select().await?;
    assert_eq!(all_books_outside_tx.len(), 3);

    // Commit the transaction
    transaction.commit().await?;

    // Verify that the books are gone (for good)
    let all_books_after_tx = Book::select().await?;
    assert_eq!(all_books_after_tx.len(), 0);

    Ok(())
}
