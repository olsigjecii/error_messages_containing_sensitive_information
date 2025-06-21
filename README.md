Test the vulnerable endpoint:

Normal Query (Expected Success):
Open your web browser and navigate to:
http://127.0.0.1:8080/vulnerable-search?product=laptop
You should see: Search Result: Successfully retrieved products for: laptop

Malicious Query (Expected Vulnerability):
Now, navigate to:
http://127.0.0.1:8080/vulnerable-search?product=book" (note the double quote at the end)
You should see an error page in your browser that explicitly includes the sensitive database connection string:


<h1>Error occurred!</h1>
<p>We encountered an issue:</p>
<pre>Database operation failed: SQL error near "". Internal details: DB_CONNECTION_STRING=postgres://admin:supersecret@localhost:5432/production_db</pre>
In your terminal where cargo run is executing, you will also see the ERROR log messages indicating the sensitive information was exposed.

----------------
FIX
----------------
In Rust, this is elegantly handled by our custom AppError type and its actix_web::error::ResponseError implementation, which we've already set up.

Secure Rust Code Pattern (from secure_search handler and AppError):

Rust

// ... (inside the secure_search Actix-Web handler) ...

// Simulates 'some code here' that might fail, returning our secure AppError type
match query_secure_database(&query.product) {
    Ok(result) => {
        // Equivalent to the `try` block succeeding
        Ok(HttpResponse::Ok().body(format!("<h1>Search Result</h1><p>{}</p>", result)))
    },
    Err(e) => { // <--- This 'e' is our 'AppError' object
        // The magic happens here: Actix-Web automatically calls 'error_response()'
        // on our 'AppError' type when 'Err(e)' is returned.
        // Inside 'error_response()':
        // 1. log_error(error_message); is handled by:
        //    error!("SECURE (internal log): Detailed DB Error: {}", details);
        //    (where 'details' are extracted from the AppError::DbError variant)
        // 2. return "An error occurred."; is handled by:
        //    HttpResponse::build(...).body("<h1>Error!</h1><p>An unexpected error occurred. Please try again later.</p>")
        Err(e) // <--- Return the AppError, letting its ResponseError impl handle the response
    }
}

// ... (And critically, the AppError's implementation looks like this) ...

impl actix_web::error::ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        // Log the detailed error for internal debugging
        match self {
            AppError::DbError(details) => {
                // This is where 'log_error(error_message)' from the JS example happens
                error!("SECURE (internal log): Detailed DB Error: {}", details);
            },
            AppError::GenericError => {
                error!("SECURE (internal log): A generic application error occurred.");
            }
        }

        // This is where 'return "An error occurred."'; from the JS example happens
        HttpResponse::build(self.status_code())
            .content_type("text/html")
            .body("<h1>Error!</h1><p>An unexpected error occurred. Please try again later.</p>")
    }

    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}
Explanation:
In the Rust code's secure_search handler, when query_secure_database encounters an issue, it returns an Err(AppError). The crucial difference from the vulnerable example is how this AppError is handled.

Instead of directly converting the raw error details into the HTTP response, we leverage Actix-Web's ResponseError trait implementation for our AppError enum.

Logging Detailed Errors (log_error(error_message);):
Within AppError::error_response(), before constructing the HTTP response for the client, we have a match self { ... } block.

If the error is an AppError::DbError(details), we explicitly use error!("SECURE (internal log): Detailed DB Error: {}", details);. This ensures that the sensitive information (like the database connection string) is written to our server-side logs, accessible only to developers for debugging, and never sent to the client.
Returning Generic Error (return "An error occurred.";):
Regardless of the specific internal error type (DbError or GenericError), the error_response() method then returns a standard HttpResponse with a generic, non-sensitive message: <body><h1>Error!</h1><p>An unexpected error occurred. Please try again later.</p>. This message provides user-friendly feedback without revealing any internal system vulnerabilities.

This approach ensures that your Actix-Web application maintains critical debugging information internally while presenting a secure, consistent, and non-revealing error message to the end-user, significantly mitigating the risk of information leakage.


