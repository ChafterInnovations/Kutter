// libs
use actix_cors::Cors;
use actix_web::http::header;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::env;
use time::{Duration, OffsetDateTime};

// structs
#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub email: String,
}

// middlewares
pub async fn create_user_table(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            username VARCHAR(255) NOT NULL UNIQUE,
            email VARCHAR(255) NOT NULL UNIQUE,
            password VARCHAR(255) NOT NULL,
            verified BOOLEAN NOT NULL DEFAULT FALSE,
            verification_code VARCHAR(6)
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub fn generate_token(username: String, email: String) -> String {
    let expiration = OffsetDateTime::now_utc() + Duration::days(1);
    let key = env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    let claims = Claims {
        sub: username,
        exp: expiration.unix_timestamp() as usize,
        email: email,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(key.as_ref()),
    )
    .unwrap();
    token
}

pub fn verify_token(token: String) -> Result<Claims, String> {
    let key = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let mut validation = Validation::default();
    validation.required_spec_claims.remove("verified");

    match decode::<Claims>(
        &token,
        &DecodingKey::from_secret(key.as_ref()),
        &Validation::default(),
    ) {
        Ok(token_data) => Ok(token_data.claims),
        Err(e) => {
            eprintln!("Token verification error: {:?}", e);
            Err("Invalid token".to_string())
        }
    }
}

pub fn cors() -> Cors {
    Cors::default()
        .allowed_origin("http://localhost:8080")
        .allowed_origin("http://localhost:1230")
        .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
        .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
        .allowed_header(header::CONTENT_TYPE)
        .max_age(3600)
}
