CREATE TABLE channels (
    platform VARCHAR(255), 
    channel VARCHAR(255),
    CONSTRAINT unique_channel PRIMARY KEY (platform, channel)
)