use std::{
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

use crate::command_handler::CommandHandler;

use super::{
    ChannelIdentifier, ChatPlatform, ChatPlatformError, Permissions, PlatformContext,
    UserIdentifier,
};

pub struct Local {
    listener: TcpListener,
    command_handler: CommandHandler,
}

#[async_trait]
impl ChatPlatform for Local {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, ChatPlatformError> {
        let addr = env::var("LOCAL_PLATFORM_ADDRESS")
            .map_err(|_| ChatPlatformError::MissingEnv(String::from("LOCAL_PLATFORM_ADDRESS")))?;

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| ChatPlatformError::ServiceError(e.to_string()))?;

        Ok(Box::new(Self {
            listener,
            command_handler,
        }))
    }

    async fn run(self) {
        tokio::spawn(async move {
            loop {
                match self.listener.accept().await {
                    Ok((stream, addr)) => {
                        let command_handler = self.command_handler.clone();

                        tokio::spawn(async move {
                            if let Err(e) =
                                Local::handle_stream(stream, addr, command_handler).await
                            {
                                tracing::warn!("Failed to handle stream: {}", e);
                            }
                        });
                    }
                    Err(e) => tracing::warn!("Failed to handle connection: {}", e),
                }
            }
        });
    }
}

impl Local {
    async fn handle_stream(
        stream: TcpStream,
        addr: SocketAddr,
        command_handler: CommandHandler,
    ) -> anyhow::Result<()> {
        let mut reader = BufReader::new(stream);
        let mut buf = String::new();

        while reader.read_line(&mut buf).await? != 0 {
            let context = LocalPlatformContext {
                addr_str: addr.ip().to_string(),
                addr,
            };

            if let Some(response) = command_handler.handle_message(&buf, context).await {
                reader.write_all(response.as_bytes()).await?;
                reader.write_all(b"\n").await?;
            }

            buf.clear();
        }

        Ok(())
    }
}

#[derive(Clone)]
struct LocalPlatformContext {
    pub addr: SocketAddr,
    pub addr_str: String,
}

#[async_trait]
impl PlatformContext for LocalPlatformContext {
    async fn get_permissions_internal(&self) -> Permissions {
        if self.addr.ip() == IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)) {
            Permissions::ChannelOwner
        } else {
            Permissions::Default
        }
    }

    fn get_channel(&self) -> ChannelIdentifier {
        ChannelIdentifier::LocalAddress(self.addr_str.clone())
    }

    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::IpAddr(self.addr.ip())
    }

    fn get_display_name(&self) -> &str {
        &self.addr_str
    }

    fn get_prefixes(&self) -> Vec<&str> {
        vec![""]
    }
}
