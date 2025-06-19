# Kutter

A real-time chat API with secure user authentication and WebSocket messaging.

## Features

- **User Authentication**: Secure register and login system with email verification
- **Real-time Chat**: WebSocket-based messaging for instant communication
- **Message Management**: View and delete your own messages
- **User Verification**: Email verification system for account security
- **Security**: JWT authentication with HTTP-only cookies and password hashing

## Tech Stack

- **Rust** with **Actix-web 4.11.0** framework
- **PostgreSQL** database with **SQLx 0.8.6** for type-safe queries
- **WebSockets** via **actix-ws** for real-time communication
- **JWT** (jsonwebtoken 9.3.1) for secure authentication
- **BCrypt** (0.17.0) for password hashing
- **Lettre** (0.11.17) for email delivery


## Prerequisites

- Rust (latest stable version)
- PostgreSQL database
- SMTP server credentials for email verification (Gmail account recommended)

## Environment Variables

Create a `.env` file in the root directory with the following variables:

```
DATABASE_URL=postgres://user:password@localhost:5432/dbname
JWT_SECRET=your_jwt_secret_key
SMTP_USER=your_email@example.com
SMTP_PSSWRD=your_email_password
```

## Installation

1. Clone the repository:
   ```
   git clone https://github.com/ChafterInnovations/Kutter.git
   cd Kutter
   ```

2. Install dependencies:
   ```
   cargo build
   ```

3. Set up the database:
   - Create a PostgreSQL database
   - Update the DATABASE_URL in your .env file
   - Tables will be created automatically on application startup

4. Run the application:
   ```
   cargo run
   ```

5. Access the application at `http://localhost:8080`

## Project Structure

- `src/`: Rust code
  - `main.rs`: Application entry point and server configuration
  - `db.rs`: Database connection and pool management
  - `middlewares.rs`: Authentication middleware and user table creation
  - `routes/`: API endpoints
    - `auth.rs`: Authentication routes (register, login, verification)
    - `chat.rs`: Chat functionality and WebSocket handling


## API Endpoints

### Authentication
- `POST /register`: Register a new user
- `POST /login`: Login with email and password
- `POST /verify_email`: Verify email with code
- `GET /verify`: Check authentication status
- `DELETE /logout`: Logout the current user

### Chat
- `GET /ws`: WebSocket endpoint for real-time chat
- `GET /messages`: Get all chat messages

## WebSocket Protocol

The WebSocket server handles message sending and deletion. The API expects the following message formats:

### Client to Server:
```json
{
  "action": "new_message",
  "payload": { "message": "Hello world!" }
}
```

```json
{
  "action": "delete_message",
  "payload": { "id": 123 }
}
```

### Server to Client:
```json
{
  "action": "new_message",
  "email": "user@example.com",
  "username": "user123",
  "message": "Hello world!",
  "time": "2023-05-20T15:30:00Z",
  "id": 123
}
```

```json
{
  "action": "delete",
  "message_id": 123
}
```

## Security Features

- Password validation: Requires minimum length, uppercase, and special characters
- Email verification: Code-based system
- JWT tokens stored in HTTP-only cookies
- Passwords hashed with BCrypt
- Input validation with regex patterns

## Development

- CORS is enabled to allow API requests from different origins
- A maintenance mode can be enabled by setting the `maintenance_mode` flag in `main.rs`
- Database tables are automatically created on application startup

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the ISC License - see the LICENSE file for details.

## About

Kutter is developed by Chafter Innovations. For questions or support, please open an issue on the repository.
