use actix::{Handler, Message};
use diesel::result::Error;
use diesel::prelude::*;
use diesel::{PgConnection};
use serde::Serialize;

use super::db_executor::DbExecutor;
use crate::models::User;

pub fn get_user(connection: &PgConnection, token_value: &str) -> Result<Option<User>, Error> {
    if token_value.len() != 32 {
        return Ok(None)
    }
    use crate::schema::users::dsl::*;
    let chapter: Option<User> = users
        .filter(token.eq(token_value))
        .first(connection)
        .optional()?;
    Ok(chapter)
}


pub struct Init {
    pub token: String,
    pub since: i64,
}
#[derive(Serialize)]
pub struct InitResult {
    pub user_name: String,
    pub display_name: String,
    pub email: Option<String>,
    pub mentions: i64,
}
impl Message for Init {
    type Result = Result<Option<InitResult>, Error>;
}
impl Handler<Init> for DbExecutor {
    type Result = Result<Option<InitResult>, Error>;
    fn handle(&mut self, msg: Init, _: &mut Self::Context) -> Self::Result {
        let connection = &self.0;
        if let Some(user) = get_user(connection, &msg.token)? {
            use crate::schema::mentions::dsl::*;
            let new_mentions: i64 = mentions
                .filter(timestamp.ge(msg.since))
                .filter(mentioned_user_id.eq(user.id))
                .count()
                .get_result(connection)?;
            Ok(Some(InitResult {
                user_name: user.user_name,
                display_name: user.display_name,
                email: user.email,
                mentions: new_mentions,
            }))
        } else {
            Ok(None)
        }
    }
}


pub struct Register {
    pub user_name: String,
    pub display_name: String,
    pub email: Option<String>,
    pub token: String,
}
pub enum RegisterResult {
    Ok,
    DuplicatedUserName,
    DuplicatedEmail,
}
impl Message for Register {
    type Result = Result<RegisterResult, Error>;
}
impl Handler<Register> for DbExecutor {
    type Result = Result<RegisterResult, Error>;
    fn handle(&mut self, msg: Register, _: &mut Self::Context) -> Self::Result {
        let connection = &self.0;
        use crate::schema::users::dsl::*;
        use diesel::dsl::*;
        if select(exists(users.filter(
            display_name.eq(&msg.display_name)
                .or(user_name.eq(&msg.user_name))
        ))).get_result(connection)? {
            return Ok(RegisterResult::DuplicatedUserName);
        }
        if let Some(user_email) = &msg.email {
            if select(exists(users.filter(email.eq(user_email)))).get_result(connection)? {
                return Ok(RegisterResult::DuplicatedEmail);
            }
        }
        insert_into(users)
            .values((
                user_name.eq(msg.user_name),
                display_name.eq(msg.display_name),
                email.eq(msg.email),
                token.eq(msg.token),
            ))
            .execute(connection)?;
        return Ok(RegisterResult::Ok);
    }
}