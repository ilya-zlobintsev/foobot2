Foobot2 is a multi-platform chat bot

## Functionality
- User-defined commands with configurable cooldowns and permissions
- Third-party service integration:
    - Twitch
    - Spotify
    - Last.FM
    - Lingva (Translation)
    - OpenWeatherMap
    - Finnhub (Stocks)
- Web UI for management
- Multi-platform user integration (can link users between platforms)
- Platform support:
    - Twitch
    - Discord
    - IRC
- WIP features:
    - Twitch EventSub (notifications for Twitch events)
    
## Development setup:

Requirements: 
- Rust
- MySQL or MariaDB with header files (-dev package)

You need to create a `.env` file that looks like this: 

*Not all variables are required, omitting them will just remove the associated funcionality. For example, not specifying Discord tokens will make the bot not connect to Discord. You should see an appropriate message in logs.*

```
BASE_URL=http://localhost:8000
DATABASE_URL=mysql://db_username:db_password@127.0.0.1/db_name
#TWITCH_LOGIN_NAME=your_bot_login_name
#TWITCH_CLIENT_ID=twitch_client_id
#TWITCH_CLIENT_SECRET=twitch_client_secret
#DISCORD_TOKEN=discord_token_here
#DISCORD_CLIENT_ID=discord_client_id
#DISCORD_CLIENT_SECRET=discord_client_secret
#SPOTIFY_CLIENT_ID=spotify_client_id
#SPOTIFY_CLIENT_SECRET=spotify_client_secret
#ADMIN_USER=twitch:twitch_admin_user_id
#OWM_API_KEY=openwweathermap_api_key
#LASTFM_API_KEY=lastmf_api_key
COMMAND_PREFIX=!
ROCKET_SECRET_KEY=rocket_secret_key_for_cookies
ALLOW_SHELL=0
#IRC_NICKNAME=irc_name
#IRC_PASSWORD=irc_password
#IRC_SERVER=irc.libera.chat
#IRC_CHANNELS="#someircchannel"
#SUPINIC_USER_ID=supinic_api_user_id
#SUPINIC_PASSWORD=supinic_api_password
#FINNHUB_API_KEY=finnhub_api_key
```

After this you can run the bot.