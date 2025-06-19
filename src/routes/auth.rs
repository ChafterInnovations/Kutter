// libs
use crate::RegexValidator;
use crate::middlewares::{generate_token, verify_token};
use actix_web::{
    HttpRequest, HttpResponse, Responder,
    cookie::{self, Cookie, SameSite},
    delete, get, post, web,
};
use bcrypt::{DEFAULT_COST, hash, verify};
use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use rand::random_range;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgPool};
use std::env;
use time::Duration;

// cookies config
fn create_cookie(token: String) -> Cookie<'static> {
    Cookie::build("token", token)
        .path("/")
        .secure(true)
        .same_site(SameSite::Lax)
        .http_only(true)
        .max_age(Duration::days(1))
        .finish()
}

fn verify_cookie(req: HttpRequest) -> Option<String> {
    req.cookie("token").map(|c| c.value().to_string())
}

const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

pub fn generate_verification_code() -> String {
    (0..6)
        .map(|_| {
            let idx = random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

// email sender
pub fn send_email(email: String, username: String, code: String) -> Result<(), String> {
    let from_address = env::var("SMTP_USER")
        .map_err(|e| format!("Failed to load SMTP_USER: {}", e))?
        .parse()
        .map_err(|e| format!("Invalid sender email format: {}", e))?;

    let to_address = email
        .parse()
        .map_err(|e| format!("Invalid recipient email format: {}", e))?;

    let email_message = Message::builder()
        .from(Mailbox::new(Some("Kutter".to_owned()), from_address))
        .to(Mailbox::new(Some(username.clone()), to_address))
        .subject("Verify your account!")
        .header(ContentType::TEXT_PLAIN)
        .body(format!(
            "Hey {}, here's your verification code: {}\n\nCopy and paste this in the app to verify your account :3",
            username, code
        ))
        .map_err(|e| format!("Failed to build email: {}", e))?;

    let creds = Credentials::new(
        env::var("SMTP_USER").map_err(|e| format!("Failed to load SMTP_USER: {}", e))?,
        env::var("SMTP_PSSWRD").map_err(|e| format!("Failed to load SMTP_PSSWRD: {}", e))?,
    );

    let mailer = SmtpTransport::relay("smtp.gmail.com")
        .map_err(|e| format!("Failed to create mailer: {}", e))?
        .credentials(creds)
        .build();

    mailer
        .send(&email_message)
        .map_err(|e| format!("Failed to send email: {}", e))?;

    Ok(())
}

// structs
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct User {
    username: String,
    email: String,
    password: String,
    verified: bool,
    verification_code: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct RegisterForm {
    username: String,
    email: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
struct LoginForm {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct VerificationData {
    email: String,
    code: String,
}

// routes
#[post("/register")]
pub async fn register(
    pool: web::Data<PgPool>,
    req: web::Json<RegisterForm>,
    validator: web::Data<RegexValidator>,
) -> impl Responder {
    let username = req.username.clone();
    let email = req.email.clone();
    let password = req.password.clone();

    if username.is_empty() || email.is_empty() || password.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "username, email, and password are required",
        }));
    }

    if !validator.email.is_match(&email) {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "invalid email format",
        }));
    }

    if !validator.username.is_match(&username) {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "username must be between 2 and 20 characters, lowercase alphabetic with _ or -",
        }));
    }

    if !validator.validate_password(&password) {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "password must be at least 6 characters long, contain at least one uppercase letter, one number, and one special character",
        }));
    }

    let password_hash = match hash(&password, DEFAULT_COST) {
        Ok(hash) => hash,
        Err(_) => {
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": "failed to hash password",
            }));
        }
    };

    let code = generate_verification_code();

    let email_exists = match sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(pool.get_ref())
        .await
    {
        Ok(user) => user.is_some(),
        Err(_) => {
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": "failed to check if email exists",
            }));
        }
    };

    if email_exists {
        return HttpResponse::Conflict().json(json!({
            "status": "error",
            "message": "email already exists",
        }));
    }

    let insert_result = sqlx::query_as::<_, User>(
        "INSERT INTO users (username, email, password, verification_code) VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(&username)
    .bind(&email)
    .bind(password_hash)
    .bind(&code)
    .fetch_one(pool.get_ref())
    .await;

    match insert_result {
        Ok(user) => {
            if let Err(e) = send_email(email, username, code) {
                return HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "message": format!("failed to send verification email: {}", e),
                }));
            }
            HttpResponse::Created().json(json!({
                "status": "success",
                "message": "user created",
                "user": user.username
            }))
        }
        Err(_) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "failed to create user",
        })),
    }
}

#[post("/login")]
pub async fn login(pool: web::Data<PgPool>, req: web::Json<LoginForm>) -> impl Responder {
    let email = req.email.clone();
    let password = req.password.clone();

    let user = match sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(pool.get_ref())
        .await
    {
        Ok(user) => user,
        Err(_) => {
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": "failed to get user",
            }));
        }
    };

    let user = match user {
        Some(user) => user,
        None => {
            return HttpResponse::Unauthorized().json(json!({
                "status": "error",
                "message": "user not found",
            }));
        }
    };

    let password_valid = match verify(&password, &user.password) {
        Ok(valid) => valid,
        Err(_) => {
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": "failed to verify password",
            }));
        }
    };

    match password_valid {
        true => {
            let token = generate_token(user.email.clone(), user.username.clone());
            let cookie = create_cookie(token);
            HttpResponse::Ok().cookie(cookie).json(json!({
                "status": "success",
                "message": "user logged in",
                "user": {
                    "username": user.username,
                    "email": user.email
                }
            }))
        }
        false => {
            return HttpResponse::Unauthorized().json(json!({
                "status": "error",
                "message": "invalid password",
            }));
        }
    }
}

#[get("/verify")]
pub async fn verify_user(req: HttpRequest, pool: web::Data<PgPool>) -> impl Responder {
    let token = match verify_cookie(req) {
        Some(token) => token,
        None => {
            return HttpResponse::Ok().json(json!({
                "status": "error",
                "message": "not authenticated"
            }));
        }
    };

    let claims = match verify_token(token) {
        Ok(claims) => claims,
        Err(_) => {
            return HttpResponse::Ok().json(json!({
                "status": "error",
                "message": "invalid token"
            }));
        }
    };

    match sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&claims.sub)
        .fetch_optional(pool.get_ref())
        .await
    {
        Ok(Some(user)) => HttpResponse::Ok().json(json!({
            "status": "success",
            "user": {
                "email": user.email,
                "username": user.username,
                "verified": user.verified
            }
        })),
        _ => HttpResponse::Ok().json(json!({
            "status": "error",
            "message": "user not found"
        })),
    }
}

#[post("/verify_email")]
pub async fn verify_email(
    pool: web::Data<PgPool>,
    req: web::Json<VerificationData>,
) -> impl Responder {
    let email = req.email.clone();
    let code = req.code.clone();

    let user = match sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(pool.get_ref())
        .await
    {
        Ok(Some(user)) => user,
        _ => {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "user not found",
            }));
        }
    };

    if user.verified {
        return HttpResponse::Conflict().json(json!({
            "status": "error",
            "message": "user already verified"
        }));
    }

    if user.verification_code.as_deref() != Some(code.as_str()) {
        return HttpResponse::Unauthorized().json(json!({
            "status": "error",
            "message": "invalid verification code"
        }));
    }

    match sqlx::query("UPDATE users SET verified = true WHERE email = $1")
        .bind(&email)
        .execute(pool.get_ref())
        .await
    {
        Ok(_) => {
            let token = generate_token(user.email.clone(), user.username.clone());
            let cookie = create_cookie(token);

            HttpResponse::Ok().cookie(cookie).json(json!({
                "status": "success",
                "message": "user verified successfully"
            }))
        }
        Err(_) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "failed to update user verification"
        })),
    }
}

#[delete("/logout")]
pub async fn logout() -> impl Responder {
    let mut cookie = Cookie::new("token", "");
    cookie.set_same_site(cookie::SameSite::Lax);
    cookie.set_secure(true);
    cookie.set_http_only(true);
    cookie.set_max_age(Duration::seconds(0));
    HttpResponse::Ok().cookie(cookie).json(json!({
        "status": "success",
        "message": "user logged out",
    }))
}
