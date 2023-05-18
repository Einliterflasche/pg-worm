use pg_worm::{connect, register, Model, NoTls};

#[derive(Model)]
struct Book {
    #[column(primary_key, auto)]
    id: i64,
    #[column(unique)]
    title: String,
}

#[tokio::test]
async fn complete_procedure() -> Result<(), pg_worm::Error> {
    // First create a connection. This can be only done _once_.
    connect!("postgres://me:me@localhost:5432", NoTls).await?;

    // Then, register the model with the pg_worm client.
    //
    // This creates a completely new table.
    // Beware that should there already be a table
    // with the same name, it is dropped.
    register!(Book).await?;

    // Next, insert a new book.
    // This works by passing values for all
    // fields which aren't autogenerated.
    Book::insert("Foo - Part I").await?;
    Book::insert("Foo - Part II").await?;

    // Query all entities from the database
    let books: Vec<Book> = Book::select().await;
    assert_eq!(books.len(), 2);
    assert_eq!(books[0].id, 1);
    assert_eq!(books[0].title, "Foo - Part I");

    let book = Book::select_one().await;
    assert!(book.is_some());

    Ok(())
}
