ALTER TABLE eventsub_triggers   
  DROP PRIMARY KEY,
  ADD PRIMARY KEY (`broadcaster_id`,`event_type`)
