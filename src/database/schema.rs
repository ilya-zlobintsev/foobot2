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

table! {
    user_data (user_id, name) {
        name -> Varchar,
        value -> Varchar,
        public -> Bool,
        user_id -> Unsigned<Bigint>,
    }
}

table! {
    web_sessions (session_id) {
        session_id -> Varchar,
        user_id -> Unsigned<Bigint>,
        username -> Text,
    }
}

joinable!(commands -> channels (channel_id));
joinable!(user_data -> users (user_id));
joinable!(web_sessions -> users (user_id));

allow_tables_to_appear_in_same_query!(
    channels,
    commands,
    users,
    user_data,
    web_sessions,
);
