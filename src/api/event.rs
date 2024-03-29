use std::ops::Deref;

use actix_web::{post, web, Either, HttpResponse, Responder};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::api::{common, user};
use crate::error::WTError;
use crate::schema::wtcup_2022_votes as wtcup_x_votes;
use crate::{AppState, DbConnection};
use actix_web::dev::HttpServiceFactory;

const MIN_CHAPTER_VOTE_ID: i16 = 70;
const MAX_CHAPTER_VOTE_ID: i16 = 87;
const VOTE_START_TIMESTAMP: i64 = 1672498800000;
const VOTE_END_TIMESTAMP: i64 = 1674918000000;

#[derive(Deserialize)]
struct VotePayload {
    token: String,
    chapter_vote_id: i16,
    rating: i16,
}

fn vote<TCon: Deref<Target = DbConnection>>(
    connection: TCon,
    token: String,
    chapter_vote_id: i16,
    rating: i16,
) -> Result<bool, WTError> {
    let user_id = user::get_user_id(&connection, &token)?;
    if let Some(user_id) = user_id {
        let affected = if rating == 0 {
            diesel::delete(wtcup_x_votes::table)
                .filter(wtcup_x_votes::user_id.eq(user_id))
                .filter(wtcup_x_votes::chapter_vote_id.eq(chapter_vote_id))
                .execute(&*connection)?
        } else {
            diesel::insert_into(wtcup_x_votes::table)
                .values((
                    wtcup_x_votes::user_id.eq(user_id),
                    wtcup_x_votes::chapter_vote_id.eq(chapter_vote_id),
                    wtcup_x_votes::rating.eq(rating),
                ))
                .on_conflict((wtcup_x_votes::user_id, wtcup_x_votes::chapter_vote_id))
                .do_update()
                .set(wtcup_x_votes::rating.eq(rating))
                .execute(&*connection)?
        };
        Ok(affected == 1)
    } else {
        Ok(false)
    }
}

#[post("/voteWtcup")]
async fn vote_handler(
    state: web::Data<AppState>,
    payload: web::Json<VotePayload>,
) -> Result<impl Responder, WTError> {
    if common::get_current_timestamp() > VOTE_END_TIMESTAMP
        || common::get_current_timestamp() < VOTE_START_TIMESTAMP
        || !user::is_token(&payload.token)
        || payload.chapter_vote_id < MIN_CHAPTER_VOTE_ID
        || payload.chapter_vote_id > MAX_CHAPTER_VOTE_ID
        || payload.rating < 0
        || payload.rating > 5
    {
        return Ok(Either::Right(HttpResponse::Forbidden()));
    }
    let connection = state.db_pool.get()?;
    if web::block(move || {
        vote(
            connection,
            payload.0.token,
            payload.0.chapter_vote_id,
            payload.0.rating,
        )
    })
    .await??
    {
        Ok(Either::Left(common::simple_success()))
    } else {
        Ok(Either::Right(HttpResponse::Forbidden()))
    }
}

#[derive(Deserialize)]
struct GetVotesPayload {
    token: String,
}

#[derive(Serialize, Queryable)]
struct GetVotesSingleResponse {
    chapter_vote_id: i16,
    rating: i16,
}

fn get_votes<TCon: Deref<Target = DbConnection>>(
    connection: TCon,
    token: String,
) -> Result<Vec<GetVotesSingleResponse>, WTError> {
    let user_id = user::get_user_id(&connection, &token)?;
    Ok(if let Some(user_id) = user_id {
        wtcup_x_votes::table
            .filter(wtcup_x_votes::user_id.eq(user_id))
            .select((wtcup_x_votes::chapter_vote_id, wtcup_x_votes::rating))
            .load(&*connection)?
    } else {
        vec![]
    })
}

#[post("/getWtcupVotes")]
async fn get_votes_handler(
    state: web::Data<AppState>,
    payload: web::Json<GetVotesPayload>,
) -> Result<impl Responder, WTError> {
    if common::get_current_timestamp() > VOTE_END_TIMESTAMP || !user::is_token(&payload.token) {
        return Ok(Either::Right(HttpResponse::Forbidden()));
    }
    let connection = state.db_pool.get()?;
    Ok(Either::Left(HttpResponse::Ok().json(
        web::block(move || get_votes(connection, payload.0.token)).await??,
    )))
}

pub fn get_service() -> impl HttpServiceFactory {
    web::scope("/event")
        .service(vote_handler)
        .service(get_votes_handler)
}
