use rocket::{Request, catch, response::content::Html};
use rocket_contrib::templates::Template;

#[catch(404)]
pub async fn not_found(_: &Request<'_>) -> Html<Template> {
    Html(Template::render("errors/404", String::new())) // String is a placeholder empty context
}
