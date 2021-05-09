table! {
    channels (platform, channel) {
        platform -> Varchar,
        channel -> Varchar,
    }
}

table! {
    users (id) {
        id -> Unsigned<Bigint>,
        twitch_id -> Nullable<Text>,
        discord_id -> Nullable<Text>,
    }
}

allow_tables_to_appear_in_same_query!(
    channels,
    users,
);
