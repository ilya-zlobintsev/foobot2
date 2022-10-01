use std::vec::IntoIter;

use super::*;
use crate::{
    command_handler::twitch_api::{
        eventsub::{
            conditions::{ChannelPointsCustomRewardRedemptionAddCondition, ChannelUpdateCondition},
            EventSubSubscriptionType,
        },
        get_client_id, get_client_secret,
        helix::HelixApi,
    },
    database::models::NewEventSubTrigger,
    platform::ChannelIdentifier,
};
use twitch_irc::login::{LoginCredentials, RefreshingLoginCredentials, StaticLoginCredentials};

pub struct TwitchEventSub {
    db: Database,
    app_api: HelixApi<StaticLoginCredentials>,
}

#[async_trait]
impl ExecutableCommand for TwitchEventSub {
    fn get_names(&self) -> &[&str] {
        &["eventsub"]
    }

    fn get_cooldown(&self) -> u64 {
        0
    }

    fn get_permissions(&self) -> Permissions {
        Permissions::ChannelMod
    }

    async fn execute<C: ExecutionContext + Send + Sync>(
        &self,
        ctx: C,
        _: &str,
        args: Vec<&str>,
        _: (&User, &UserIdentifier),
    ) -> Result<Option<String>, CommandError> {
        if let ChannelIdentifier::TwitchChannel((broadcaster_id, _)) = ctx.get_channel() {
            let mut args = args.into_iter();
            let action = args
                .next()
                .ok_or_else(|| CommandError::MissingArgument("action".to_owned()))?;

            match action {
                "add" | "create" => {
                    let (subscription, action) =
                        self.get_subscription(args, broadcaster_id.clone()).await?;

                    if action.is_empty() {
                        return Err(CommandError::MissingArgument("action".to_owned()));
                    }

                    let subscription_response = self
                        .app_api
                        .add_eventsub_subscription(subscription.clone())
                        .await
                        .map_err(|e| {
                            CommandError::GenericError(format!(
                                "Failed to create subscription: {}",
                                e
                            ))
                        })?;

                    let id = &subscription_response.data.first().unwrap().id;

                    self.db.add_eventsub_trigger(NewEventSubTrigger {
                        broadcaster_id: &broadcaster_id,
                        event_type: subscription.get_type(),
                        action: &action,
                        creation_payload: &serde_json::to_string(&subscription)
                            .expect("failed to serialize"),
                        id,
                    })?;

                    Ok(Some("Trigger successfully added".to_owned()))
                }
                "remove" | "delete" => {
                    let (subscription_type, _) =
                        self.get_subscription(args, broadcaster_id.clone()).await?;

                    let subscriptions = self
                        .app_api
                        .get_eventsub_subscriptions(Some(subscription_type.get_type()))
                        .await?;

                    if let Some(subscription) = subscriptions
                        .iter()
                        .find(|sub| sub.condition == subscription_type.get_condition())
                    {
                        self.app_api
                            .delete_eventsub_subscription(&subscription.id)
                            .await?;
                        self.db.delete_eventsub_trigger(&subscription.id)?;

                        Ok(Some("Trigger succesfully removed".to_owned()))
                    } else {
                        Err(CommandError::InvalidArgument(
                            "unable to find matching subscription".to_owned(),
                        ))
                    }
                }
                "list" => {
                    let triggers = self
                        .db
                        .get_eventsub_triggers_for_broadcaster(&broadcaster_id)?;

                    if !triggers.is_empty() {
                        let output = triggers
                            .into_iter()
                            .map(|trigger| trigger.event_type)
                            .collect::<Vec<String>>()
                            .join(", ");
                        Ok(Some(output))
                    } else {
                        Ok(Some("No eventsub triggers registered".to_owned()))
                    }
                }
                _ => Err(CommandError::GenericError(format!(
                    "invalid action {action}"
                ))),
            }
        } else {
            Err(CommandError::GenericError(
                "EventSub can only be used on Twitch".into(),
            ))
        }
    }
}

impl TwitchEventSub {
    async fn get_subscription(
        &self,
        mut args: IntoIter<&str>,
        broadcaster_id: String,
    ) -> Result<(EventSubSubscriptionType, String), CommandError> {
        let sub_type = args
            .next()
            .ok_or_else(|| CommandError::MissingArgument("subscription type".to_owned()))?;
        let mut action = args.collect::<Vec<&str>>().join(" ");

        let subscription = match sub_type {
            "channel.update" => EventSubSubscriptionType::ChannelUpdate(ChannelUpdateCondition {
                broadcaster_user_id: broadcaster_id.clone(),
            }),
            "channel.channel_points_custom_reward_redemption.add" | "points.redeem" => {
                let action_clone = action.clone();

                let (reward_name, action_str) = match action_clone.split_once(';') {
                    Some((reward_name, action_str)) => (reward_name, action_str),
                    None => (action_clone.as_str(), ""),
                };

                tracing::info!("Searching for reward {}", reward_name);

                action = action_str.trim().to_string();
                let reward_name = reward_name.trim();

                let streamer_credentials = self.db.make_twitch_credentials(broadcaster_id.clone());
                let refreshing_credentials = RefreshingLoginCredentials::init(
                    get_client_id().unwrap(),
                    get_client_secret().unwrap(),
                    streamer_credentials,
                );

                refreshing_credentials
                    .get_credentials()
                    .await
                    .map_err(|_| {
                        CommandError::GenericError(
                            "streamer has not authenticated the bot to manage channel points"
                                .to_owned(),
                        )
                    })?;

                let streamer_api = HelixApi::with_credentials(refreshing_credentials).await;

                let rewards_response = streamer_api.get_custom_rewards().await?;

                let reward = rewards_response
                    .data
                    .iter()
                    .find(|reward| reward.title.trim() == reward_name)
                    .ok_or_else(|| {
                        CommandError::InvalidArgument(format!(
                            "could not find reward `{reward_name}`"
                        ))
                    })?;

                EventSubSubscriptionType::ChannelPointsCustomRewardRedemptionAdd(
                    ChannelPointsCustomRewardRedemptionAddCondition {
                        broadcaster_user_id: broadcaster_id,
                        reward_id: Some(reward.id.clone()),
                    },
                )
            }
            _ => {
                return Err(CommandError::InvalidArgument(format!(
                    "Invalid subscription type {}",
                    sub_type
                )))
            }
        };

        Ok((subscription, action))
    }
}

impl TwitchEventSub {
    pub fn new(db: Database, app_api: HelixApi<StaticLoginCredentials>) -> Self {
        Self { db, app_api }
    }
}
