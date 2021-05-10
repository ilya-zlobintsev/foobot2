table! {
    channels (id) {
        id -> Unsigned<Bigint>,
        platform -> Varchar,
        channel -> Varchar,
    }
}

table! {
    commands (channel_id, name) {
        name -> Varchar,
        action -> Text,
        permissions -> Nullable<Text>,
        channel_id -> Unsigned<Bigint>,
    }
}

table! {
    users (id) {
        id -> Unsigned<Bigint>,
        twitch_id -> Nullable<Text>,
        discord_id -> Nullable<Text>,
    }
}

joinable!(commands -> channels (channel_id));

allow_tables_to_appear_in_same_query!(
    channels,
    commands,
    users,
);
