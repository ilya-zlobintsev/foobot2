CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    twitch_id TEXT UNIQUE,
    discord_id TEXT UNIQUE
)