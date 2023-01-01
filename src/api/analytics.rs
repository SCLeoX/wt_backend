use actix_web::dev::HttpServiceFactory;
use actix_web::{get, post, web, Either, HttpResponse, Responder};
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Bigint, VarChar};
use diesel::{insert_into, sql_query};
use indoc::indoc;
use serde::{Deserialize, Serialize};

use crate::error::WTError;
use crate::models::Chapter;
use crate::schema::{chapters, visits};
use crate::{AppState, DbConnection};

use super::common;

const PAGE_SIZE: i32 = 50;

#[derive(Deserialize, Copy, Clone)]
enum TimeFrame {
    HOUR,
    DAY,
    WEEK,
    MONTH,
    YEAR,
}

impl TimeFrame {
    fn get_milliseconds(self) -> i64 {
        match self {
            TimeFrame::HOUR => 1000 * 3600,
            TimeFrame::DAY => 1000 * 3600 * 24,
            TimeFrame::WEEK => 1000 * 3600 * 24 * 7,
            TimeFrame::MONTH => 1000 * 3600 * 24 * 30,
            TimeFrame::YEAR => 1000 * 3600 * 24 * 365,
        }
    }
}

fn count(connection: &DbConnection, relative_path: &str) -> Result<(), WTError> {
    connection.transaction::<(), WTError, _>(|| {
        let chapter = common::get_chapter(&connection, &relative_path)?;
        insert_into(visits::table)
            .values((
                visits::chapter_id.eq(chapter.id),
                visits::timestamp.eq(common::get_current_timestamp()),
            ))
            .execute(connection)?;
        diesel::update(&chapter)
            .set(chapters::visit_count.eq(chapters::visit_count + 1))
            .execute(connection)?;
        Ok(())
    })
}

#[post("/count")]
async fn count_handler(
    state: web::Data<AppState>,
    content: String,
) -> Result<Either<impl Responder, impl Responder>, WTError> {
    if !common::is_page_name(&content) {
        return Ok(Either::Left(HttpResponse::Forbidden()));
    }
    let connection = state.db_pool.get()?;
    web::block(move || count(&connection, &content)).await??;
    Ok(Either::Right(HttpResponse::Ok().body("<3")))
}

#[derive(Serialize)]
struct OneChapterVisitInfo {
    relative_path: String,
    visit_count: i64,
}

type ChapterVisitInfo = Vec<OneChapterVisitInfo>;

#[derive(Deserialize)]
struct ChapterAllQuery {
    page: i32,
}

#[get("/chapters/all")]
async fn chapter_all_handler(
    state: web::Data<AppState>,
    query: web::Query<ChapterAllQuery>,
) -> Result<impl Responder, WTError> {
    let connection = state.db_pool.get()?;
    let statement = chapters::table
        .order(chapters::visit_count.desc())
        .offset(((query.page - 1) * PAGE_SIZE).into())
        .limit(PAGE_SIZE.into());
    let showing_chapters: Vec<Chapter> =
        web::block(move || statement.load::<Chapter>(&connection)).await??;
    let chapter_visit_info: ChapterVisitInfo = showing_chapters
        .into_iter()
        .map(|showing_chapter| OneChapterVisitInfo {
            visit_count: showing_chapter.visit_count,
            relative_path: showing_chapter.relative_path,
        })
        .collect();
    Ok(HttpResponse::Ok().json(chapter_visit_info))
}

#[get("/chapters/allRaw")]
async fn chapter_all_raw_handler(state: web::Data<AppState>) -> Result<impl Responder, WTError> {
    let connection = state.db_pool.get()?;
    let showing_chapters: Vec<Chapter> =
        web::block(move || chapters::table.load(&connection)).await??;
    let chapter_visit_info: ChapterVisitInfo = showing_chapters
        .into_iter()
        .map(|showing_chapter| OneChapterVisitInfo {
            visit_count: showing_chapter.visit_count,
            relative_path: showing_chapter.relative_path,
        })
        .collect();
    Ok(HttpResponse::Ok().json(chapter_visit_info))
}

#[derive(Deserialize)]
struct ChapterRecentQuery {
    page: i32,
    time_frame: TimeFrame,
}

#[get("/chapters/recent")]
async fn chapter_recent_handler(
    state: web::Data<AppState>,
    query: web::Query<ChapterRecentQuery>,
) -> Result<impl Responder, WTError> {
    let connection = state.db_pool.get()?;

    #[derive(QueryableByName)]
    struct RecentAggregateResult {
        #[sql_type = "VarChar"]
        relative_path: String,
        #[sql_type = "BigInt"]
        visit_count: i64,
    }
    let sql = indoc! {"
        SELECT chapters.relative_path, count(1) as visit_count FROM visits
            LEFT JOIN chapters
                ON visits.chapter_id = chapters.id
            WHERE visits.timestamp > $1
            GROUP BY chapters.id
            ORDER BY visit_count DESC
            LIMIT $2
            OFFSET $3
    "};
    let statement = sql_query(sql)
        .bind::<Bigint, i64>(common::get_current_timestamp() - query.time_frame.get_milliseconds())
        .bind::<Bigint, i64>(PAGE_SIZE.into())
        .bind::<Bigint, i64>(((query.page - 1) * PAGE_SIZE).into());
    let showing_chapters: Vec<RecentAggregateResult> =
        web::block(move || statement.get_results(&connection)).await??;
    let chapter_visit_info: ChapterVisitInfo = showing_chapters
        .into_iter()
        .map(|showing_chapter| OneChapterVisitInfo {
            visit_count: showing_chapter.visit_count,
            relative_path: showing_chapter.relative_path,
        })
        .collect();
    Ok(HttpResponse::Ok().json(chapter_visit_info))
}

pub fn get_service() -> impl HttpServiceFactory {
    web::scope("/stats")
        .service(count_handler)
        .service(chapter_all_handler)
        .service(chapter_all_raw_handler)
        .service(chapter_recent_handler)
}
