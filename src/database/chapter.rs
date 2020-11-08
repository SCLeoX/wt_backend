use diesel::{insert_into};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::result::Error;

use crate::models::Chapter;

pub fn get_chapter(connection: &PgConnection, relative_path_value: &str) -> Result<Chapter, Error> {
    use crate::schema::chapters::dsl::*;
    let chapter: Option<Chapter> = chapters
        .filter(relative_path.eq(relative_path_value))
        .first(connection)
        .optional()?;
    if let Some(chapter) = chapter {
        Ok(chapter)
    } else {
        let row: Chapter = insert_into(chapters)
            .values(relative_path.eq(relative_path_value))
            .get_result(connection)?;
        Ok(row)
    }
}
