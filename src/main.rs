// libs
use actix_web::{App, HttpServer};
use actix_files as fs;
use dotenv::dotenv;
use std::sync::Arc;
use tokio::sync::broadcast;
use routes::AppState;

// mods
pub mod routes;
pub mod middlewares;
pub mod db;


// main
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let pool = db::create_pool().await;
    let (tx, _) = broadcast::channel(20);

    let app_state = Arc::new(AppState {
        db_pool: pool.clone(),
        tx,
    });

    routes::create_table(&pool).await.expect("Failed to create table");

    HttpServer::new(move || {
        App::new()
            .app_data(actix_web::web::Data::new(pool.clone()))
            .app_data(actix_web::web::Data::new(app_state.clone()))
            .wrap(middlewares::cors())
            .service(routes::ws_handler)
            .service(routes::get_messages)
            .service(fs::Files::new("/", "./static").index_file("index.html"))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}