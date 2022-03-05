use std::env;

use crate::command_handler::{platform_handler::PlatformHandlerError, CommandHandler};
use tonic::{transport::Server, Request, Response, Status};

use self::foobot::foobot_server::{Foobot, FoobotServer};
use foobot::{EchoRequest, EchoResponse};

pub mod foobot {
    tonic::include_proto!("foobot");
}

pub fn start_server(command_handler: CommandHandler) {
    tokio::spawn(async move {
        let port = env::var("GRPC_PORT").unwrap_or_else(|_| String::from("50051"));
        let addr = format!("0.0.0.0:{}", port).parse().unwrap();
        tracing::info!("GRPC server is listening on {}", addr);

        if let Err(e) = Server::builder()
            .add_service(FoobotServer::new(FoobotService { command_handler }))
            .serve(addr)
            .await
        {
            tracing::error!("GRPC server error: {}", e);
        }
    });
}

#[derive(Debug)]
struct FoobotService {
    command_handler: CommandHandler,
}

#[tonic::async_trait]
impl Foobot for FoobotService {
    async fn send_message(
        &self,
        request: Request<EchoRequest>,
    ) -> Result<Response<EchoResponse>, Status> {
        let request = request.into_inner();
        tracing::info!("{:?}", request);

        let channel = self
            .command_handler
            .db
            .get_channel_by_id(request.channel_id)
            .expect("DB error")
            .ok_or_else(|| Status::not_found("Specified channel not found"))?;

        let platform_handler = self.command_handler.platform_handler.read().await;

        match platform_handler
            .send_to_channel(channel.get_identifier(), request.message)
            .await
        {
            Ok(()) => Ok(Response::new(EchoResponse {})),
            Err(PlatformHandlerError::Unsupported) => Err(Status::unimplemented(
                "Remotely sending messages is not supported for this platform",
            )),
            Err(PlatformHandlerError::Unconfigured) => Err(Status::unavailable(
                "Target channel's platform is not configured",
            )),
            Err(PlatformHandlerError::PlatformError(e)) => Err(Status::internal(e.to_string())),
        }
    }
}
