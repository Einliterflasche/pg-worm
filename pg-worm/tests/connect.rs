#![allow(dead_code)]

use pg_worm::prelude::*;
use tokio::try_join;

#[derive(Model)]
struct Book {
    #[column(primary_key, auto)]
    id: i64,
    #[column(unique)]
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
        Book::insert(
            "Foo - Part I",
            "Subtitle".to_string(),
            vec!["Page 1".to_string()],
            1
        ),
        Book::insert("Foo - Part II", None, vec![], 2),
        Book::insert("Foo - Part III", None, vec![], 3)
    )?;

    // Let's start with a simple query for all books.
    let all_books = Book::select().await?;
    assert_eq!(all_books.len(), 3);

    // Or select based on a condition
    let books_with_subtitle = Book::select().filter(Book::sub_title.not_null()).await?;
    assert_eq!(books_with_subtitle.len(), 1);

    // Or select just one book
    let first_book = Book::select_one().filter(Book::id.eq(1)).await?;
    assert!(first_book.is_some());

    // Or delete all books without a subtitle
    let book_deleted = Book::delete()
        .filter(Book::sub_title.null())
        .await?;
    assert_eq!(book_deleted, 2);

    let books_updated = Book::update()
        .set(Book::title, "trololol")
        .await?;   
    assert_eq!(books_updated, 1);

    Ok(())
}
