table! {
    chapters (id) {
        id -> Int4,
        relative_path -> Varchar,
        visit_count -> Int8,
    }
}

table! {
    comments (id) {
        id -> Int8,
        chapter_id -> Int4,
        user_id -> Int8,
        content -> Varchar,
        deleted -> Bool,
        create_timestamp -> Int8,
        update_timestamp -> Int8,
    }
}

table! {
    mentions (id) {
        id -> Int8,
        from_comment_id -> Int8,
        mentioned_user_id -> Int8,
        timestamp -> Int8,
    }
}

table! {
    users (id) {
        id -> Int8,
        token -> Bpchar,
        email -> Nullable<Varchar>,
        user_name -> Varchar,
        display_name -> Varchar,
        disabled -> Bool,
        last_checked_mentions_timestamp -> Int8,
    }
}

table! {
    visits (id) {
        id -> Int8,
        chapter_id -> Int4,
        timestamp -> Int8,
    }
}

joinable!(comments -> users (user_id));
joinable!(comments -> chapters (chapter_id));
joinable!(mentions -> comments (from_comment_id));
joinable!(mentions -> users (mentioned_user_id));

allow_tables_to_appear_in_same_query!(
    chapters,
    comments,
    mentions,
    users,
    visits,
);
