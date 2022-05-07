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

You need to copy the `.env.dist` file to `.env` and change/uncomment the needed values.

*Not all variables are required, omitting them will just remove the associated funcionality. For example, not specifying Discord tokens will make the bot not connect to Discord. You should see an appropriate message in logs.*

After this you can run the bot.