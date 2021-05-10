use super::schema::*;

#[derive(Queryable, Debug)]
pub struct User {
    pub id: u64,
    pub twitch_id: Option<String>,
    pub discord_id: Option<String>,
}

#[derive(Insertable, Default)]
#[table_name="users"]
pub struct NewUser<'a> {
    pub twitch_id: Option<&'a str>,
    pub discord_id: Option<&'a str>,
}

#[derive(Queryable, Debug, PartialEq, Eq)]
pub struct Channel {
    pub id: u64,
    pub platform: String,
    pub channel: String,
}

#[derive(Insertable)]
#[table_name="channels"]
pub struct NewChannel<'a> {
    pub platform: &'a str,
    pub channel: &'a str,
}

#[derive(Queryable, Debug, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    pub action: String,
    pub permissions: Option<String>,
    pub channel_id: u64, 
}

#[derive(Insertable, Debug, PartialEq, Eq)]
#[table_name="commands"]
pub struct NewCommand<'a> {
    pub name: &'a str,
    pub action: &'a str,
    pub permissions: Option<&'a str>,
    pub channel_id: u64, 
}