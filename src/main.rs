#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate lazy_static;

use std::env;

use actix_cors::Cors;
use actix_web::{App, HttpServer};
use diesel::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use dotenv::dotenv;

pub mod schema;
mod models;
mod api;
mod error;
mod dark_colors;

embed_migrations!();

pub type DbConnection = PgConnection;

struct AppState {
    db_pool: Pool<ConnectionManager<DbConnection>>,
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<DbConnection>::new(database_url);
    let db_pool = Pool::new(manager).expect("Failed to create pool.");
    embedded_migrations::run(&db_pool.get().expect("Failed to obtain connection for migration.")).expect("Migration failed.");
    HttpServer::new(move || {
        let cors = Cors::default()
            // .allowed_origin("http://127.0.0.1:2333")
            // .allowed_origin("http://localhost:2333")
            .allowed_origin("https://wt.tepis.me")
            .allowed_origin("https://wt.bgme.me")
            .allowed_origin("https://rbq.desi")
            .allowed_origin("https://wt.makai.city")
            .allowed_origin("https://wt.0w0.bid")
            .allowed_origin("https://wt.umwings.com")
            .allowed_methods(vec!["GET", "POST"])
            .allowed_header("Content-Type")
            .max_age(3600);
        App::new()
            .data(AppState { db_pool: db_pool.clone() })
            .wrap(cors)
            .service(api::analytics::get_service())
            .service(api::user::get_service())
            .service(api::comment::get_service())
            .service(api::event::get_service())
    })
        .bind("127.0.0.1:8088")?
        .run()
        .await
}
