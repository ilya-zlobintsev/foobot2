use anyhow::anyhow;
use minecraft_client_rs::Client;
use std::env;

pub fn init() -> anyhow::Result<Client> {
    let address = env::var("MINECRAFT_RCON_ADDRESS")?;
    let password = env::var("MINECRAFT_RCON_PASSWORD")?;

    let mut client = Client::new(address).map_err(|e| anyhow!("{}", e))?;

    client
        .authenticate(password)
        .map_err(|e| anyhow!("Failed to authenticate Minecraft RCON: {}", e))?;

    Ok(client)
}
