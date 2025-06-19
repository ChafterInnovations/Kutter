use actix_files as fs;
use actix_web::{App, HttpResponse, HttpServer, web};
use dotenv::dotenv;
use regex::Regex;
use routes::chat::AppState;
use std::fs as std_fs;
use std::sync::Arc;
use tokio::sync::broadcast;

pub mod db;
pub mod middlewares;
pub mod routes;

#[derive(Clone)]
pub struct RegexValidator {
    pub email: Regex,
    pub username: Regex,
    pub password: Regex,
}

impl RegexValidator {
    pub fn new() -> Self {
        Self {
            email: Regex::new(r"^[\w\.-]+@[\w\.-]+\.\w{2,}$").unwrap(),
            username: Regex::new(r"^[a-z0-9_-]{2,20}$").unwrap(),
            password: Regex::new(r"^.{6,}$").unwrap(),
        }
    }

    pub fn validate_password(&self, password: &str) -> bool {
        if !self.password.is_match(password) {
            return false;
        }

        let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
        let has_special = password.chars().any(|c| !c.is_ascii_alphanumeric());

        has_upper && has_special
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let pool = db::create_pool().await;
    let (tx, _) = broadcast::channel(20);
    let regex_validator = RegexValidator::new();

    let app_state = Arc::new(AppState {
        db_pool: pool.clone(),
        tx,
    });

    middlewares::create_user_table(&pool)
        .await
        .expect("Failed to create table");

    routes::chat::create_table(&pool)
        .await
        .expect("Failed to create table");

    let maintenance_mode = false; // !!!!!

    HttpServer::new(move || {
        let app = App::new()
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(app_state.clone()))
            .app_data(web::Data::new(regex_validator.clone()))
            .wrap(middlewares::cors());

        if maintenance_mode {
            app.default_service(web::route().to(|| async {
                let html = std_fs::read_to_string("./static/maintain.html").unwrap_or_else(|_| {
                    String::from("<h1>Site em manutenção</h1><p>Voltaremos em breve!</p>")
                });

                HttpResponse::ServiceUnavailable()
                    .content_type("text/html")
                    .body(html)
            }))
        } else {
            app.service(routes::auth::register)
                .service(routes::auth::login)
                .service(routes::auth::verify_user)
                .service(routes::chat::ws_handler)
                .service(routes::chat::get_messages)
                .service(routes::auth::verify_email)
                .service(routes::auth::logout)
                .service(fs::Files::new("/", "./static").index_file("index.html"))
        }
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
