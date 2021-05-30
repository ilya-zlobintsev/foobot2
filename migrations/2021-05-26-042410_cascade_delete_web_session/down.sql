ALTER TABLE web_sessions DROP FOREIGN KEY web_sessions_ibfk_1;
ALTER TABLE web_sessions ADD FOREIGN KEY (user_id) REFERENCES users(id);