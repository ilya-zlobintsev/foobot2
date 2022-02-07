ALTER TABLE eventsub_triggers   
  DROP PRIMARY KEY,
  ADD UNIQUE (`broadcaster_id`,`event_type`,`creation_payload`)
