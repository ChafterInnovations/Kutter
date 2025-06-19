// libs
use crate::middlewares::verify_token;
use actix_web::{Error, HttpRequest, HttpResponse, Responder, get, web};
use actix_ws::Message;
use chrono::{DateTime, Utc};
use futures_util::StreamExt as _;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::sync::Arc;
use tokio::sync::broadcast;

// structs
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatMessage {
    pub id: Option<i32>,
    pub email: String,
    pub username: String,
    pub message: String,
    pub time: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewMessage {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub action: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct DeleteMessageRequest {
    pub id: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum OutgoingMessage {
    NewMessage(ChatMessage),
    Delete { message_id: i32 },
}

pub struct AppState {
    pub db_pool: PgPool,
    pub tx: broadcast::Sender<OutgoingMessage>,
}

// mods
pub async fn create_table(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS messages (
            id SERIAL PRIMARY KEY,
            email VARCHAR(255) NOT NULL REFERENCES users(email),
            username VARCHAR(255) NOT NULL REFERENCES users(username),
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
    let token = match req.cookie("token") {
        Some(token) => token.value().to_string(),
        None => return Ok(HttpResponse::Unauthorized().finish()),
    };

    let claims = match verify_token(token) {
        Ok(claims) => claims,
        Err(_) => return Ok(HttpResponse::Unauthorized().finish()),
    };

    let email = claims.sub.clone();
    let username = claims.email.clone();

    let (response, session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    let db_pool = state.db_pool.clone();
    let tx = state.tx.clone();
    let mut rx = tx.subscribe();

    let mut broadcast_session = session.clone();
    let mut message_session = session;

    actix_rt::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Err(e) = broadcast_session
                .text(serde_json::to_string(&msg).unwrap())
                .await
            {
                eprintln!("Error sending WS broadcast: {}", e);
                break;
            }
        }
    });

    actix_rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(ws_msg) = serde_json::from_str::<WebSocketMessage>(&text) {
                        match ws_msg.action.as_str() {
                            "new_message" => {
                                if let Ok(new_msg) =
                                    serde_json::from_value::<NewMessage>(ws_msg.payload)
                                {
                                    match sqlx::query_as::<_, ChatMessage>(
                                        "INSERT INTO messages (email, username, message) VALUES ($1, $2, $3) RETURNING *"
                                    )
                                    .bind(&email)
                                    .bind(&username)
                                    .bind(&new_msg.message)
                                    .fetch_one(&db_pool)
                                    .await {
                                        Ok(saved_msg) => {
                                            let _ = tx.send(OutgoingMessage::NewMessage(saved_msg));
                                        },
                                        Err(e) => eprintln!("Error saving message: {:?}", e),
                                    }
                                }
                            }
                            "delete_message" => {
                                if let Ok(delete_req) =
                                    serde_json::from_value::<DeleteMessageRequest>(ws_msg.payload)
                                {
                                    match sqlx::query_as::<_, ChatMessage>(
                                        "SELECT id, email, username, message, time FROM messages WHERE id = $1"
                                    )
                                    .bind(delete_req.id)
                                    .fetch_optional(&db_pool)
                                    .await {
                                        Ok(Some(msg)) => {
                                            if msg.email != email {
                                                let error_response = serde_json::json!({
                                                    "status": "error",
                                                    "message": "You can only delete your own messages"
                                                });
                                                let _ = message_session.text(serde_json::to_string(&error_response).unwrap()).await;
                                                continue;
                                            }

                                            match sqlx::query("DELETE FROM messages WHERE id = $1")
                                                .bind(delete_req.id)
                                                .execute(&db_pool)
                                                .await {
                                                Ok(_) => {
                                                    let broadcast = OutgoingMessage::Delete {
                                                        message_id: delete_req.id,
                                                    };
                                                    let _ = tx.send(broadcast);
                                                }
                                                Err(e) => {
                                                    eprintln!("Error deleting message: {:?}", e);
                                                }
                                            }
                                        },
                                        Ok(None) => {
                                            let error_response = serde_json::json!({
                                                "status": "error",
                                                "message": "Message not found"
                                            });
                                            let _ = message_session.text(serde_json::to_string(&error_response).unwrap()).await;
                                        },
                                        Err(e) => {
                                            eprintln!("Error fetching message: {}", e);
                                        }
                                    }
                                }
                            }
                            _ => eprintln!("Unknown action: {}", ws_msg.action),
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
        "SELECT id, email, username, message, time FROM messages ORDER BY time DESC",
    )
    .fetch_all(&state.db_pool)
    .await
    {
        Ok(messages) => HttpResponse::Ok().json(messages),
        Err(e) => {
            eprintln!("Error fetching messages: {}", e);
            HttpResponse::InternalServerError().json("Error fetching messages")
        }
    }
}
