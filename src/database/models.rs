#[derive(Queryable, Debug)]
pub struct User {
    pub id: u64,
    pub twitch_username: Option<String>,
    pub discord_id: Option<String>,
}
