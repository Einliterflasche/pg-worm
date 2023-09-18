//! This contains a lighter and slightly adapted version of `deadpool-postgres`.
use std::{
    ops::{Deref, DerefMut},
    str::FromStr,
    sync::{Arc, Mutex, OnceLock},
};

use deadpool::managed::{self, Object};
use hashbrown::HashMap;
use once_cell::sync::Lazy;
use tokio::{self, task::JoinHandle};
use tokio_postgres::{
    tls::{MakeTlsConnect, TlsConnect},
    Client as PgClient, Config as PgConfig, NoTls, Socket, Statement,
};

use crate::Error;

static POOL: OnceLock<Pool> = OnceLock::new();
/// This is a single client which is used for prepared statements.
static PREPARED_CLIENT: OnceLock<Client> = OnceLock::new();
/// This hashmap keeps track of all prepared statements.
static PREPARED_STATEMENTS: Lazy<Arc<Mutex<HashMap<String, Statement>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

/// The pool which houses all connections to the PostgreSQL sever.
type Pool = managed::Pool<Manager>;
/// A wrapper around connections to make them poolable.
pub type Client = Object<Manager>;
/// A unit struct which only provides the `build` method.
pub struct Connection;

/// A wrapper around a [tokio_postgres::Client] with a spawned off `Connection`.
pub struct ClientWrapper {
    inner: PgClient,
    conn_handle: JoinHandle<()>,
}

/// The pool manager which creates/recycles Clients when they are returned/destroyed.
pub struct Manager {
    config: PgConfig,
    connector: Box<dyn Connect + Send + Sync>,
}

#[doc(hidden)]
#[async_trait::async_trait]
pub trait Connect {
    async fn connect(&self, _: &PgConfig) -> Result<(PgClient, JoinHandle<()>), crate::Error>;
}

struct Connector<Tls> {
    tls: Tls,
}

/// A struct for building a connection pool according to your needs.
pub struct ConnectionBuilder {
    conn_string: String,
}

/// Try to fetch a client from the connection pool.
#[doc(hidden)]
#[inline]
pub async fn fetch_client() -> Result<Client, Error> {
    POOL.get()
        .ok_or(Error::NotConnected)?
        .get()
        .await
        .map_err(|_| Error::NoConnectionInPool)
}

#[doc(hidden)]
#[inline]
pub async fn fetch_prepared_client() -> Result<&'static Client, Error> {
    PREPARED_CLIENT.get().ok_or(Error::NotConnected)
}

#[doc(hidden)]
#[inline]
pub async fn ensure_prepared(statement: &str) -> Result<(), Error> {
    let is_prepared = PREPARED_STATEMENTS
        .lock()
        .map_err(|_| Error::NotConnected)?
        .contains_key(statement);

    if is_prepared {
        return Ok(());
    }

    let prepared_stmt = fetch_prepared_client().await?.prepare(statement).await?;
    let owned_stmt = statement.to_string();

    PREPARED_STATEMENTS
        .lock()
        .map_err(|_| Error::NotConnected)?
        .insert(owned_stmt, prepared_stmt);

    Ok(())
}

/// Hidden function so set the pool from the `config` module.
#[doc(hidden)]
pub fn set_pool(pool: Pool) -> Result<(), Error> {
    POOL.set(pool).map_err(|_| Error::AlreadyConnected)
}

impl Connection {
    /// Start building the connection/pool.
    pub fn build(connection_string: impl Into<String>) -> ConnectionBuilder {
        ConnectionBuilder {
            conn_string: connection_string.into(),
        }
    }
}

impl ConnectionBuilder {
    /// Finish building and set up the pool. Does not actually connect until
    /// the first `Client`s are retrieved.
    pub fn connect(self) -> Result<(), Error> {
        let pg_config =
            PgConfig::from_str(&self.conn_string).map_err(|_| Error::InvalidPoolConfig)?;

        let manager = Manager::new(pg_config);

        let pool = Pool::builder(manager)
            .build()
            .map_err(|_| Error::InvalidPoolConfig)?;
        set_pool(pool)
    }

    /// Set the maximum amount of Connections in the pool.
    ///
    /// Default: `num_cpus * 4`.
    pub fn max_pool_size(self, _n: usize) -> ConnectionBuilder {
        self
    }
}

impl Manager {
    fn new(pg_config: PgConfig) -> Manager {
        Self {
            config: pg_config,
            connector: Box::new(Connector { tls: NoTls }),
        }
    }
}

#[async_trait::async_trait]
impl managed::Manager for Manager {
    type Type = ClientWrapper;
    type Error = crate::Error;

    async fn create(&self) -> Result<ClientWrapper, crate::Error> {
        let (client, handle) = self.connector.connect(&self.config).await?;
        Ok(ClientWrapper {
            inner: client,
            conn_handle: handle,
        })
    }

    async fn recycle(&self, client: &mut ClientWrapper) -> managed::RecycleResult<crate::Error> {
        if client.is_closed() {
            return Err(managed::RecycleError::StaticMessage(
                "client couldn't be recycled as it's closed",
            ));
        }

        Ok(())
    }
}

impl Deref for ClientWrapper {
    type Target = PgClient;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ClientWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Drop for ClientWrapper {
    fn drop(&mut self) {
        self.conn_handle.abort();
    }
}

#[async_trait::async_trait]
impl<T> Connect for Connector<T>
where
    T: MakeTlsConnect<Socket> + Clone + Sync + Send + 'static,
    T::Stream: Sync + Send,
    T::TlsConnect: Sync + Send,
    <T::TlsConnect as TlsConnect<Socket>>::Future: Send,
{
    async fn connect(&self, config: &PgConfig) -> Result<(PgClient, JoinHandle<()>), crate::Error> {
        // Create a new connection
        let (client, conn) = config.connect(self.tls.clone()).await?;
        // "Start" the connection by spawning a new thread
        let handle = tokio::spawn(async move {
            // Swallow potential errors for now
            let _ = conn.await;
        });

        Ok((client, handle))
    }
}
