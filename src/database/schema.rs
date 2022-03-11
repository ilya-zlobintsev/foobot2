table! {
    auth (name) {
        name -> Varchar,
        value -> Nullable<Varchar>,
    }
}

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
        cooldown -> Nullable<Unsigned<Bigint>>,
        triggers -> Nullable<Text>,
    }
}

table! {
    eventsub_triggers (id) {
        broadcaster_id -> Varchar,
        event_type -> Varchar,
        action -> Mediumtext,
        creation_payload -> Longtext,
        id -> Varchar,
    }
}

table! {
    filters (channel_id, regex) {
        channel_id -> Unsigned<Bigint>,
        regex -> Varchar,
        block_message -> Bool,
        replacement -> Nullable<Varchar>,
    }
}

table! {
    mirror_connections (from_channel_id, to_channel_id) {
        from_channel_id -> Unsigned<Bigint>,
        to_channel_id -> Unsigned<Bigint>,
    }
}

table! {
    prefixes (channel_id) {
        channel_id -> Unsigned<Bigint>,
        prefix -> Tinytext,
    }
}

table! {
    users (id) {
        id -> Unsigned<Bigint>,
        twitch_id -> Nullable<Text>,
        discord_id -> Nullable<Text>,
        irc_name -> Nullable<Text>,
        local_addr -> Nullable<Text>,
        telegram_id -> Nullable<Text>,
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
joinable!(filters -> channels (channel_id));
joinable!(prefixes -> channels (channel_id));
joinable!(user_data -> users (user_id));
joinable!(web_sessions -> users (user_id));

allow_tables_to_appear_in_same_query!(
    auth,
    channels,
    commands,
    eventsub_triggers,
    filters,
    mirror_connections,
    prefixes,
    users,
    user_data,
    web_sessions,
);
