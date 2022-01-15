CREATE TABLE eventsub_triggers(
    channel_id BIGINT UNSIGNED,
    event_type VARCHAR(255),
    action MEDIUMTEXT,
    FOREIGN KEY (channel_id) REFERENCES channels (id),
    PRIMARY KEY (channel_id, event_type)
)