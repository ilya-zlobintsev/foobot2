CREATE TABLE channels (
    id SERIAL PRIMARY KEY,
    platform VARCHAR(255) NOT NULL, 
    channel VARCHAR(255) NOT NULL,
    CONSTRAINT unique_channel UNIQUE (platform, channel)
)