CREATE TABLE filters (
    channel_id BIGINT UNSIGNED,
    regex VARCHAR(255) NOT NULL, 
    block_message BOOLEAN NOT NULL,
    replacement VARCHAR(255),
    PRIMARY KEY(channel_id, regex),
    FOREIGN KEY (channel_id) REFERENCES channels(id)
)