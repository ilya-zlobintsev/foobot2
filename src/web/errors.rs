use rocket::{
    catch,
    response::{content::Html, Redirect},
    Request,
};
use rocket_dyn_templates::Template;

#[catch(404)]
pub async fn not_found(_: &Request<'_>) -> Html<Template> {
    Html(Template::render("errors/404", String::new())) // String is a placeholder empty context
}

#[catch(401)]
pub async fn not_authorized(_: &Request<'_>) -> Redirect {
    Redirect::to("/authenticate")
}
