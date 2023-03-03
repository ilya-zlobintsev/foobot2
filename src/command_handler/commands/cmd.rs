use super::*;
use crate::{api::get_base_url, database::DatabaseError};

pub struct Cmd;

#[async_trait]
impl ExecutableCommand for Cmd {
    fn get_names(&self) -> &[&str] {
        &[
            "cmd", "addcmd", "delcmd", "editcmd", "showcmd", "help", "commands",
        ]
    }

    fn get_cooldown(&self) -> u64 {
        0
    }

    fn get_permissions(&self) -> Permissions {
        Permissions::Default // There is custom permission handling per-subcommand
    }

    async fn execute<'a, P: PlatformContext + Send + Sync>(
        &self,
        ctx: &ExecutionContext<'a, P>,
        mut trigger_name: &str,
        mut args: Vec<&str>,
    ) -> Result<Option<String>, CommandError> {
        let channel_identifier = ctx.platform_ctx.get_channel();
        let channel = ctx
            .db
            .get_or_create_channel(&channel_identifier)?
            .ok_or(CommandError::NoPermissions)?; // Shouldn't happen anyway

        match trigger_name {
            "addcmd" => {
                trigger_name = "cmd";
                args.insert(0, "add");
            }
            "delcmd" => {
                trigger_name = "cmd";
                args.insert(0, "delete");
            }
            "editcmd" => {
                trigger_name = "cmd";
                args.insert(0, "edit");
            }
            "showcmd" => {
                trigger_name = "cmd";
                args.insert(0, "show");
            }
            "help" => {
                args.clear();
            }
            _ => (),
        }

        let mut arguments = args.into_iter();

        let response = if arguments.len() == 0 {
            Ok(Some(format!(
                "{}/channels/{}/commands",
                get_base_url(),
                channel.id,
            )))
        } else if ctx.get_permissions().await? >= Permissions::ChannelMod {
            match arguments.next().ok_or_else(|| {
                CommandError::MissingArgument("must be either add or delete".to_string())
            })? {
                "add" | "create" => {
                    let mut command_name = arguments
                        .next()
                        .ok_or_else(|| CommandError::MissingArgument("command name".to_string()))?;

                    for prefix in ctx.platform_ctx.get_prefixes() {
                        if let Some(stripped_name) = command_name.strip_prefix(prefix) {
                            command_name = stripped_name;
                        }
                    }

                    let command_action = arguments.collect::<Vec<&str>>().join(" ");

                    if command_action.is_empty() {
                        return Err(CommandError::MissingArgument("command action".to_string()));
                    }

                    match ctx.db.add_command_to_channel(
                        &channel_identifier,
                        command_name,
                        &command_action,
                    ) {
                        Ok(()) => Ok(Some("Command successfully added".to_string())),
                        Err(DatabaseError::DieselError(diesel::result::Error::DatabaseError(
                            diesel::result::DatabaseErrorKind::UniqueViolation,
                            _,
                        ))) => Ok(Some("Command already exists".to_string())),
                        Err(e) => Err(CommandError::DatabaseError(e)),
                    }
                }
                "del" | "delete" | "remove" => {
                    let mut command_name = arguments
                        .next()
                        .ok_or_else(|| CommandError::MissingArgument("command name".to_string()))?;

                    if let Some(stripped_name) = command_name.strip_prefix('!') {
                        command_name = stripped_name;
                    }

                    match ctx
                        .db
                        .delete_command_from_channel(&channel_identifier, command_name)
                    {
                        Ok(()) => Ok(Some("Command succesfully removed".to_string())),
                        Err(e) => Err(CommandError::DatabaseError(e)),
                    }
                }
                "edit" | "update" => {
                    let command_name = arguments
                        .next()
                        .ok_or_else(|| CommandError::MissingArgument("command name".to_string()))?;
                    let command_action = arguments.collect::<Vec<&str>>().join(" ");

                    if command_action.is_empty() {
                        return Err(CommandError::MissingArgument("command action".to_string()));
                    }

                    match ctx.db.update_command_action(
                        &channel_identifier,
                        command_name,
                        &command_action,
                    ) {
                        Ok(()) => Ok(Some(format!("Command {command_name} updated"))),
                        Err(e) => Err(CommandError::DatabaseError(e)),
                    }
                }
                "show" | "check" => {
                    let mut command_name = arguments
                        .next()
                        .ok_or_else(|| CommandError::MissingArgument("command name".to_string()))?;

                    if let Some(stripped_name) = command_name.strip_prefix('!') {
                        command_name = stripped_name;
                    }

                    match ctx.db.get_command(&channel_identifier, command_name)? {
                        Some(command) => Ok(Some(command.action)),
                        None => Ok(Some(format!("command {} doesn't exist", command_name))),
                    }
                }
                "set_triggers" => {
                    let mut command_name = arguments
                        .next()
                        .ok_or_else(|| CommandError::MissingArgument("command name".to_string()))?;

                    if let Some(stripped_name) = command_name.strip_prefix('!') {
                        command_name = stripped_name;
                    }

                    let triggers = arguments.collect::<Vec<&str>>().join(" ");

                    if triggers.is_empty() {
                        return Err(CommandError::MissingArgument("triggers".to_string()));
                    }

                    ctx.db
                        .set_command_triggers(channel.id, command_name, &triggers)?;

                    Ok(Some(String::from("Succesfully updated command triggers")))
                }
                "get_triggers" => {
                    let mut command_name = arguments
                        .next()
                        .ok_or_else(|| CommandError::MissingArgument("command name".to_string()))?;

                    if let Some(stripped_name) = command_name.strip_prefix('!') {
                        command_name = stripped_name;
                    }

                    let commands = ctx.db.get_commands(channel.id)?;

                    for command in commands {
                        if command.name == command_name {
                            return Ok(match command.triggers {
                                Some(triggers) => Some(format!("Command triggers: {}", triggers)),
                                None => Some(String::from("Command has no triggers")),
                            });
                        }
                    }
                    Ok(Some(String::from("Command not found")))
                }
                _ => Err(CommandError::InvalidArgument(trigger_name.to_owned())),
            }
        } else {
            Err(CommandError::NoPermissions)
        }?;

        // TODO
        // self.refresh_command_triggers(channel.id)?;

        Ok(response)
    }
}
