# Introduction

Foobot has a small amount of built-in commands for managing the bot. Most of the bot's functionality is used as part of custom, per-channel commands.

## Built-in commands:

- **ping** - gets current bot status, uptime etc
- **whoami**/**id** - get user information about yourself
- **debug** (mods+) - execute a *command action*
- **cmd/addcmd/delcmd/showcmd** - see below
- **eventsub** (mods+) - manage eventsub (Twitch only), see below

## Managing commands

Mods or channel owners can manage commands with either **cmd subcommand** or with their aliases(if available). List of subcommands:
- **add** (alias: addcmd)
- **delete** (alias: delcmd)
- **edit** (alias: editcmd)
- **show** (alias: showcmd)
- **set_triggers**
- **get_triggers**

The basic commands are self-explanatory, with the syntax being: **cmd add commandname commandaction**

Command triggers allow you to fire the command when a certain phrase is used, not just when the command is explicitly called. Triggers are fired if the message starts with the given phrase. Triggers are specified with a semicolon-separated list of words/phrases. For example:

**cmd set_triggers mycommand hello1;hello2**

Now **mycommand** will be triggered every time someone types a  message starting with hello1 or hello2.

## Command action

When defining a command, you specify the command's response - a command action. The command action can be simple text, but it's also rendered as a [**Handlebars template**](https://handlebarsjs.com/guide/) (specifically using the [Rust version](https://github.com/sunng87/handlebars-rust/)). 

Example command action greeting the user: 
```
Hello {{ display_name }}!
```

Handlebars is a powerful templating engine which allows you to specify advanced command logic. For the actual language language syntax you should look at [its documentation](https://handlebarsjs.com/). 

### Data
The following data is directly available within templates: 

- arguments - array, command arguments
- display_name - string, the display name of the user
- user - object, the user using the command. Has following fields:
- id - the id of the user within the bot 
- an _id field for every possible platform, for example: **twitch_id** or **discord_id**

### Helpers

Helpers can be thought of as functions within templates. They are used in the same way as normal variables, for example:
```
You are currently listening to: {{ song }}
```
Handlebars ships a number of built-in helpers, and the Rust version includes some [extra ones](https://github.com/sunng87/handlebars-rust/blob/master/src/helpers/helper_extras.rs#L6). However, the main foobot functionality is contained within custom helpers:

- args - get the command arguments as a singular string

- say - send a message in the calling message separate from the command response. Can be useful in combination with helpers such as **sleep**

- translate - translates text using the [lingva](https://github.com/thedaviddelta/lingva-translate) API. Defaults to auto-detecting the source language and setting the target language to english, that can be configured using **to:** and **from:** arguments.

- spotify - get current Spotify song for the user
- spotify_last_song - get the last played Spotify song for the user
- spotify_playlist - get the Spotify playlist the user is currenty listening to
- lastfm - get the current song from the user's last.fm
- song - get the user's current song, attempting to fetch it from last.fm first and then spotify, whichever is configured

- choose - get a random option from the given arguments
- sleep - wait for the given number of seconds
- concat - join the arguments

- stock - get the current stock price from the given symbol

- weather - get current weather for the given location

- forsencode_encode - encode [forsencode](https://gist.githubusercontent.com/GaZaTu/ca2e6e1c9abd8b2da35b9b2d73919ac8/raw/cfbef5546a6da64d90c9e90d13d2c385b416fc31/forsencode-rfc.txt)
- forsencode_decode - see above

- twitchuser - get information about a Twitch user, returns an object in the format of the [Twitch API](https://dev.twitch.tv/docs/api/reference#get-users). If used on Twitch, will default to the calling user if none is specified.

- get - make an http request for the given url, only plaintext or json is allowed
- json - deserialize a given json, can be used together with get

- data_set - set temporary channel data with a given key and value
- data_get - fetch temporary chanenl data

- rhai - TODO

Helpers can be nested:
```
{{ weather (args) }}
```
This would get weather for the location specified in the command arguments.


## EventSub


TODO
