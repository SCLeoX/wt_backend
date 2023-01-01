#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate lazy_static;

use std::env;

use actix_cors::Cors;
use actix_web::{App, HttpServer};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use dotenv::dotenv;

mod api;
mod dark_colors;
mod error;
mod models;
pub mod schema;

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
    embedded_migrations::run(
        &db_pool
            .get()
            .expect("Failed to obtain connection for migration."),
    )
    .expect("Migration failed.");
    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://127.0.0.1:2333")
            .allowed_origin("http://localhost:2333")
            .allowed_origin("https://wt.tepis.me")
            .allowed_origin("https://wt.bgme.bid")
            .allowed_origin("https://wt.bgme.me")
            .allowed_origin("https://rbq.desi")
            .allowed_origin("https://wt.0w0.bid")
            .allowed_origin("https://wt.umwings.com")
            .allowed_origin("https://wt.innocent.love")
            .allowed_origin("https://wt.mirror.0w0.bid")
            .allowed_origin("http://rintepis.gitgud.site")
            .allowed_methods(vec!["GET", "POST"])
            .allowed_header("Content-Type")
            .max_age(3600);
        App::new()
            .app_data(actix_web::web::Data::new(AppState {
                db_pool: db_pool.clone(),
            }))
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
