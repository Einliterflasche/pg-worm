//! This module contains the code for configuring the connection pool.

use std::str::FromStr;

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::{
    tls::{MakeTlsConnect, TlsConnect},
    Config, NoTls, Socket,
};

/// An empty struct that only provides the `build` method.
pub struct Connection;

///
pub struct ConnectionBuilder<Tls>
where
    Tls: MakeTlsConnect<Socket> + Clone + Send + Sync + 'static,
    Tls::Stream: Sync + Send,
    Tls::TlsConnect: Sync + Send,
    <Tls::TlsConnect as TlsConnect<Socket>>::Future: Send,
{
    url: String,
    tls: Tls,
    recycling_method: RecyclingMethod,
    max_pool_size: Option<usize>,
    runtime: Option<Runtime>,
}

impl<Tls> ConnectionBuilder<Tls>
where
    Tls: MakeTlsConnect<Socket> + Clone + Send + Sync + 'static,
    Tls::Stream: Sync + Send,
    Tls::TlsConnect: Sync + Send,
    <Tls::TlsConnect as TlsConnect<Socket>>::Future: Send,
{
    fn to(url: impl Into<String>) -> ConnectionBuilder<NoTls> {
        ConnectionBuilder {
            url: url.into(),
            tls: NoTls,
            recycling_method: RecyclingMethod::Fast,
            max_pool_size: None,
            runtime: None,
        }
    }

    /// Set the Tls method.
    ///
    /// Use either [`postgres-openssl`](https://crates.io/crates/postgres-openssl)
    /// or [`postgres-nativ-tls`](https://crates.io/crates/postgres-native-tls)
    /// and their respective documentation.
    /// This function accepts the same types as `tokio-postgres`.
    pub fn tls<NewTls>(self, tls: NewTls) -> ConnectionBuilder<NewTls>
    where
        NewTls: MakeTlsConnect<Socket> + Clone + Send + Sync + 'static,
        NewTls::Stream: Sync + Send,
        NewTls::TlsConnect: Sync + Send,
        <NewTls::TlsConnect as TlsConnect<Socket>>::Future: Send,
    {
        ConnectionBuilder {
            tls,
            url: self.url,
            recycling_method: self.recycling_method,
            runtime: self.runtime,
            max_pool_size: self.max_pool_size,
        }
    }

    /// Set the maximum size of the connection pool,
    /// i.e. the maximum amount of concurrent connections to the database server.
    ///
    /// The default is `num_cpus * 4`, ignoring hyperthreading, etc.
    pub fn max_pool_size(mut self, n: usize) -> Self {
        self.max_pool_size = Some(n);

        self
    }

    /// Finish the setup and build the pool.
    ///
    /// Fails if
    ///  - the url couldn't be parsed, or
    ///  - some other configuration error has been made.
    pub fn connect(self) -> Result<(), crate::Error> {
        let config = Config::from_str(&self.url)?;
        let manager_config = ManagerConfig {
            recycling_method: self.recycling_method,
        };

        let manager = Manager::from_config(config, self.tls, manager_config);
        let mut builder = Pool::builder(manager).runtime(Runtime::Tokio1);

        if let Some(n) = self.max_pool_size {
            builder = builder.max_size(n);
        }

        let pool = builder.build()?;

        crate::set_pool(pool)?;

        Ok(())
    }
}

impl Connection {
    /// Start building a new connection (pool).
    ///
    /// This returns a [`ConnectionBuilder`] which can be configured
    /// using the builder pattern.
    ///
    /// If you are fine with the default configuration
    /// (`max_pool_size = num_cpus * 4` and no Tls) or have
    /// configured to your needs you  can finish the setup
    /// by calling `.connect()`.
    ///  
    /// A connection must be created before executing any
    /// queries or the like.
    /// Doing otherwise will result in a runime error.
    ///
    /// # Example
    /// ```ignore
    /// use pg_worm::prelude::*;
    ///
    /// Connection::build("postgres://postgres").connect()?;    
    /// ```
    ///
    pub fn build(connection_string: impl Into<String>) -> ConnectionBuilder<NoTls> {
        ConnectionBuilder::<NoTls>::to(connection_string)
    }
}
