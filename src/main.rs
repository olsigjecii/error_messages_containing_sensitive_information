use actix_web::{App, HttpResponse, HttpServer, Responder, http::StatusCode, web};
use env_logger::Env; // For initializing the logger
use log::{error, info}; // For logging messages
use serde::Deserialize;
use std::fmt; // For formatting errors

// =========================================================================
// --- Simulated Database Error (Vulnerable - Kept for comparison) ---
// =========================================================================

// This error will carry sensitive information directly in its Display implementation.
#[derive(Debug)]
struct VulnerableDbError {
    details: String,
}

impl fmt::Display for VulnerableDbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Database operation failed: {}", self.details)
    }
}

// A simple function that simulates a database query.
// It's designed to fail and reveal sensitive info if a specific input is given.
fn query_vulnerable_database(input: &str) -> Result<String, VulnerableDbError> {
    if input.contains('"') {
        // Simulate a malformed query that triggers a detailed internal error
        let sensitive_info =
            "DB_CONNECTION_STRING=postgres://admin:supersecret@localhost:5432/production_db";
        error!(
            "VULNERABLE (internal log): SQL query error with input '{}'. Details: {}",
            input, sensitive_info
        );
        Err(VulnerableDbError {
            details: format!(
                "SQL error near \"{}\". Internal details: {}",
                input, sensitive_info
            ),
        })
    } else {
        Ok(format!("Successfully retrieved products for: {}", input))
    }
}

// =========================================================================
// --- Request Body/Query Parameter Struct ---
// =========================================================================

#[derive(Deserialize)]
struct SearchQuery {
    product: String,
}

// =========================================================================
// --- Actix-Web Handler for the Vulnerable Endpoint (Kept for comparison) ---
// =========================================================================

// This handler will directly return the vulnerable error message to the client.
async fn vulnerable_search(query: web::Query<SearchQuery>) -> impl Responder {
    info!("Received vulnerable search request for: {}", query.product);
    match query_vulnerable_database(&query.product) {
        Ok(result) => HttpResponse::Ok().body(format!("<h1>Search Result</h1><p>{}</p>", result)),
        Err(e) => {
            // VULNERABLE: Returning the detailed error message directly in the HTTP response.
            error!(
                "VULNERABLE (client response): Exposing error to client: {}",
                e
            ); // Also log it for server-side visibility
            HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                .content_type("text/html")
                .body(format!(
                    "<h1>Error occurred!</h1><p>We encountered an issue:</p><pre>{}</pre>",
                    e.to_string()
                ))
        }
    }
}

// =========================================================================
// --- SECURE ERROR HANDLING IMPLEMENTATION ---
// =========================================================================

// 1. Define a Custom Error Type
// This enum will represent different types of application errors.
// Crucially, it allows us to store sensitive details internally (e.g., `DbError`)
// but provide a generic user-facing message.
#[derive(Debug)]
enum AppError {
    // This variant stores the actual detailed database error message,
    // which should ONLY be logged internally.
    DbError(String),
    // This variant is for generic errors that we want to show to the user.
    GenericError,
    // You could add more specific error types here (e.g., NotFound, Unauthorized)
}

// Implement `std::fmt::Display` for `AppError` if you want to print it,
// but remember this might expose details if used carelessly for client responses.
// For secure handling, we'll control the output in `Responder` implementation.
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::DbError(details) => write!(f, "Internal Database Error: {}", details),
            AppError::GenericError => write!(f, "An unexpected application error occurred."),
        }
    }
}

// 2. Implement `actix_web::error::ResponseError` for our Custom Error Type
// This trait tells Actix-Web how to convert `AppError` into an `HttpResponse`.
impl actix_web::error::ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        // Log the detailed error for internal debugging
        match self {
            AppError::DbError(details) => {
                // This will log the sensitive information, but only on the server side.
                error!("SECURE (internal log): Detailed DB Error: {}", details);
            }
            AppError::GenericError => {
                error!("SECURE (internal log): A generic application error occurred.");
            }
        }

        // Return a generic, non-sensitive message to the client
        HttpResponse::build(self.status_code())
            .content_type("text/html")
            .body("<h1>Error!</h1><p>An unexpected error occurred. Please try again later.</p>")
    }

    fn status_code(&self) -> StatusCode {
        // All application errors will return a 500 Internal Server Error
        // to the client, as we don't want to leak specific error types.
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

// 3. Secure Database Query Function
// This function now returns our custom `AppError` type.
fn query_secure_database(input: &str) -> Result<String, AppError> {
    if input.contains('"') {
        // Simulate a malformed query that triggers an internal error
        let sensitive_info =
            "DB_CONNECTION_STRING=postgres://admin:supersecret@localhost:5432/production_db";
        // When an error occurs, create an `AppError::DbError` variant
        // storing the sensitive details.
        Err(AppError::DbError(format!(
            "SQL error near \"{}\". Internal details: {}",
            input, sensitive_info
        )))
    } else {
        Ok(format!("Successfully retrieved products for: {}", input))
    }
}

// 4. Actix-Web Handler for the Secure Endpoint
// This handler now returns `Result<HttpResponse, AppError>`.
// When `AppError` is returned, Actix-Web will use our `ResponseError`
// implementation to generate the HTTP response, ensuring sensitive data is not leaked.
async fn secure_search(query: web::Query<SearchQuery>) -> Result<HttpResponse, AppError> {
    info!("Received secure search request for: {}", query.product);
    match query_secure_database(&query.product) {
        Ok(result) => {
            Ok(HttpResponse::Ok().body(format!("<h1>Search Result</h1><p>{}</p>", result)))
        }
        Err(e) => {
            // Actix-Web will automatically call `e.error_response()`
            // and `e.status_code()` to create the response.
            // Our implementation logs details and returns generic message.
            Err(e)
        }
    }
}

// =========================================================================
// --- Main Function to Run the Actix-Web Server ---
// =========================================================================

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging. Set RUST_LOG=info or RUST_LOG=error to control verbosity.
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    info!("Starting Actix-Web server on http://127.0.0.1:8080");

    HttpServer::new(|| {
        App::new()
            // Home route
            .route("/", web::get().to(|| async { HttpResponse::Ok().body("<h1>Welcome! Try /vulnerable-search?product=test or /secure-search?product=test</h1>") }))
            // Vulnerable endpoint (for comparison)
            .service(web::resource("/vulnerable-search").route(web::get().to(vulnerable_search)))
            // Secure endpoint
            .service(web::resource("/secure-search").route(web::get().to(secure_search)))
            // Default 404 handler for unmatched routes
            .default_service(web::to(|| async { HttpResponse::NotFound().body("<h1>404 Not Found</h1>") }))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
