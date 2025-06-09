// libs
use actix_web::{get, web, Error, HttpRequest, HttpResponse, Responder};
use actix_ws::Message;
use futures_util::StreamExt as _;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, FromRow};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::broadcast;

// structs
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatMessage {
    pub id: Option<i32>,
    pub message: String,
    pub time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct NewMessage {
    pub message: String,
}

pub struct AppState {
    pub db_pool: PgPool,
    pub tx: broadcast::Sender<ChatMessage>,
}

// mods
pub async fn create_table(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS messages (
            id SERIAL PRIMARY KEY,
            message TEXT NOT NULL,
            time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

// routes
#[get("/ws")]
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<Arc<AppState>>,
) -> Result<HttpResponse, Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    let db_pool = state.db_pool.clone();
    let tx = state.tx.clone();
    let mut rx = tx.subscribe();

    actix_rt::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Err(e) = session.text(serde_json::to_string(&msg).unwrap()).await {
                eprintln!("❌ Error sending WS message: {}", e);
                break;
            }
        }
    });

    actix_rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                Message::Text(text) => {
                    println!("✅ Received message: {}", text);

                    if let Ok(new_msg) = serde_json::from_str::<NewMessage>(&text) {
                        if let Ok(saved) = sqlx::query_as::<_, ChatMessage>(
                            "INSERT INTO messages (message) VALUES ($1) RETURNING *"
                        )
                        .bind(&new_msg.message)
                        .fetch_one(&db_pool)
                        .await {
                            let _ = tx.send(saved);
                        }
                    }
                }
                _ => {}
            }
        }
    });

    Ok(response)
}

#[get("/messages")]
pub async fn get_messages(state: web::Data<Arc<AppState>>) -> impl Responder {
    match sqlx::query_as::<_, ChatMessage>(
        "SELECT * FROM messages ORDER BY time DESC"
    )
    .fetch_all(&state.db_pool)
    .await {
        Ok(messages) => HttpResponse::Ok().json(messages),
        Err(e) => {
            eprintln!("❌ Error fetching messages: {}", e);
            HttpResponse::InternalServerError().json("Error fetching messages")
        }
    }
}
