mod template_context;
mod channel;
mod errors;

use rocket::{catchers, get, routes};
use rocket_contrib::templates::Template;
use tokio::task::{self, JoinHandle};

use template_context::*;

use crate::database::Database;

#[get("/")]
fn index() -> String {
    "Hello".to_string()
}

pub async fn run(db: Database) -> JoinHandle<()> {
    task::spawn(async {
        rocket::build()
            .manage(db)
            .attach(Template::fairing())
            .mount("/", routes![index])
            .mount("/channels", routes![channel::index, channel::commands_page])
            .register("/", catchers![errors::not_found])
            .register("/channels", catchers![channel::not_found])
            .launch()
            .await
            .expect("Failed to launch web server")
    })
}
