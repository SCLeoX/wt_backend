#[macro_use]
extern crate diesel;

use actix_cors::Cors;

use actix::Addr;
use actix_web::{App, FromRequest, HttpResponse, HttpServer, Responder, web};
use serde::{Deserialize, Serialize};

use database::DbExecutor;
use error::WTError;

use crate::database::{get_db_executor, ListChaptersAll, RecordVisit, TimeFrame, ListChapterRecent, Init, Register, RegisterResult};
use rand::Rng;

pub mod schema;
mod models;
mod database;
mod error;

async fn count_handler(state: web::Data<AppState>, content: String) -> Result<impl Responder, WTError> {
    state.db.send(RecordVisit { relative_path: content }).await??;
    Ok(HttpResponse::Ok().body("<3"))
}

#[derive(Deserialize)]
struct ChapterAllQuery {
    page: i32,
}
async fn chapter_all_handler(state: web::Data<AppState>, query: web::Query<ChapterAllQuery>) -> Result<impl Responder, WTError> {
    let result = state.db.send(ListChaptersAll { page: query.page }).await??;
    Ok(HttpResponse::Ok().json(result))
}

#[derive(Deserialize)]
struct ChapterRecentQuery {
    page: i32,
    time_frame: TimeFrame,
}
async fn chapter_recent_handler(state: web::Data<AppState>, query: web::Query<ChapterRecentQuery>) -> Result<impl Responder, WTError> {
    let result = state.db.send(ListChapterRecent { page: query.page, time_frame: query.time_frame }).await??;
    Ok(HttpResponse::Ok().json(result))
}

#[derive(Deserialize)]
struct InitQuery {
    token: String,
    since: i64,
}
async fn init_handler(state: web::Data<AppState>, query: web::Query<InitQuery>) -> Result<impl Responder, WTError> {
    if let Some(result) = state.db.send(Init { token: query.0.token, since: query.0.since }).await?? {
        Ok(HttpResponse::Ok().json(result))
    } else {
        Ok(HttpResponse::Forbidden().body("Invalid token"))
    }
}

#[derive(Deserialize)]
struct RegisterData {
    user_name: String,
    email: Option<String>,
}
#[derive(Serialize)]
#[serde(untagged)]
enum RegisterResponse {
    Ok { success: bool, token: String },
    Err { success: bool, code: i32 },
}
async fn register_handler(state: web::Data<AppState>, payload: web::Json<RegisterData>) -> Result<impl Responder, WTError> {
    let token: String = rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(32)
        .collect();
    let display_name = payload.user_name.replace(' ', "_");
    match state.db.send(Register {
        user_name: payload.0.user_name,
        display_name,
        email: payload.0.email,
        token: token.clone(),
    }).await?? {
        RegisterResult::Ok => {
            Ok(HttpResponse::Ok().json(RegisterResponse::Ok {
                success: true,
                token,
            }))
        }
        RegisterResult::DuplicatedEmail => {
            Ok(HttpResponse::Forbidden().json(RegisterResponse::Err {
                success: false,
                code: 1,
            }))
        }
        RegisterResult::DuplicatedUserName => {
            Ok(HttpResponse::Forbidden().json(RegisterResponse::Err {
                success: false,
                code: 2,
            }))
        }
    }
}

struct AppState {
    db: Addr<DbExecutor>
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let db_addr = get_db_executor();
    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://127.0.0.1:2333")
            .allowed_origin("http://localhost:2333")
            .allowed_origin("https://wt.tepis.me")
            .allowed_origin("https://wt.bgme.me")
            .allowed_origin("https://rbq.desi")
            .allowed_origin("https://wt.makai.city")
            .allowed_origin("https://wt.0w0.bid")
            .max_age(3600);
        App::new()
            .wrap(cors)
            .data(AppState {
                db: db_addr.clone(),
            })
            .service(
                web::resource("/count")
                    .app_data(String::configure(|cfg| {
                        cfg.limit(1024)
                    }))
                    .route(web::post().to(count_handler))
            )
            .route(
                "/stats/chapters/all",
                web::get().to(chapter_all_handler),
            )
            .route(
                "/stats/chapters/recent",
                web::get().to(chapter_recent_handler),
            )
            .route(
                "/init",
                web::get().to(init_handler),
            )
            .route(
                "/register",
                web::post().to(register_handler),
            )
    })
        .bind("127.0.0.1:8088")?
        .run()
        .await
}
