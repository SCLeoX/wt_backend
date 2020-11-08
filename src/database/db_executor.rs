use std::env;

use actix::{Actor, Addr, SyncArbiter, SyncContext};
use diesel::{Connection};
use diesel::pg::PgConnection;
use dotenv::dotenv;

pub struct DbExecutor(pub PgConnection);

impl Actor for DbExecutor {
    type Context = SyncContext<Self>;
}

pub fn get_db_executor() -> Addr<DbExecutor> {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    SyncArbiter::start(4, move || {
        DbExecutor(PgConnection::establish(&database_url)
            .unwrap_or_else(|_| panic!("Error connecting to {}.", database_url)))
    })
}

