CREATE TABLE eventsub_triggers(
    broadcaster_id VARCHAR(255) NOT NULL,
    event_type VARCHAR(255) NOT NULL,
    action MEDIUMTEXT NOT NULL,
    PRIMARY KEY (broadcaster_id, event_type)
)