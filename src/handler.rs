use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use minijinja::Environment;
use tracing::{info, warn};

use crate::content_type;
use crate::openrouter::{
    ChatCompletionRequest, Message, MessageRole, OpenRouterClient, ProviderPrefs, ProviderSort,
};
use crate::state::AppState;
use crate::utils::normalize_path;

/// Creates a minijinja environment with error page templates
fn create_template_env() -> Environment<'static> {
    let mut env = Environment::new();

    env.add_template(
        "build_request_error",
        "<h1>Error generating page</h1><p>Failed to build request: {{ error }}</p>",
    )
    .expect("Failed to add build_request_error template");

    env.add_template(
        "api_error",
        "<h1>Error generating page</h1><p>{{ error }}</p>",
    )
    .expect("Failed to add api_error template");

    env
}

/// Determines the content type based on the request method, Accept header, and path.
fn determine_content_type<'a>(
    method: &Method,
    headers: &HeaderMap,
    path: &str,
    state: &'a AppState,
) -> Result<(&'a str, &'a crate::config::ContentTypeConfig), Box<Response>> {
    if method == Method::POST {
        // For POST requests, always generate JSON regardless of path
        match state
            .config
            .content_types
            .iter()
            .find(|(mime, _)| mime.as_str() == "application/json")
        {
            Some((mime, config)) => Ok((mime.as_str(), config)),
            None => {
                info!("application/json not configured");
                Err(Box::new(
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "application/json not configured",
                    )
                        .into_response(),
                ))
            }
        }
    } else {
        // For GET and other requests, check Accept header first, then fall back to path
        let accept_header = headers.get("accept").and_then(|v| v.to_str().ok());

        match content_type::determine_from_accept(accept_header, &state.config)
            .or_else(|| content_type::determine_from_path(path, &state.config))
        {
            Some((mime, ct)) => Ok((mime.as_str(), ct)),
            None => {
                // Unsupported file extension and no Accept header match
                Err(Box::new(
                    (StatusCode::NOT_FOUND, "Not Found").into_response(),
                ))
            }
        }
    }
}

/// Builds reference materials from database-stored referer, base page, parent paths, and request body.
async fn build_reference_materials(
    state: &AppState,
    referer: &str,
    uri: &Uri,
    path: &str,
    method: &Method,
    body_str: &str,
) -> String {
    let mut reference_materials = String::new();

    // Build reference materials from database-stored referer if available
    if !referer.is_empty() {
        // Extract path and query from referer URL
        if let Ok(referer_url) = referer.parse::<Uri>() {
            let referer_path = normalize_path(referer_url.path());
            let referer_query = referer_url.query().unwrap_or("");

            if let Ok(Some(referer_content)) = state.db.get(referer_path, referer_query).await {
                reference_materials.push_str("### ");
                reference_materials.push_str(referer_path);
                reference_materials.push_str("\n\n");
                reference_materials.push_str(&referer_content);
                info!(referer = %referer_path, "Loaded referer content from database");
            }
        }
    }

    // Include base page content if this is a query variation
    // e.g., for /apples?color=green, include /apples if available
    if let Some(query_str) = uri.query()
        && !query_str.is_empty()
        && let Ok(Some(base_content)) = state.db.get(path, "").await
    {
        if !reference_materials.is_empty() {
            reference_materials.push_str("\n\n");
        } else {
            reference_materials.push_str("## Reference materials\n\n");
        }
        reference_materials.push_str("### ");
        reference_materials.push_str(path);
        reference_materials.push_str(" (base page)\n\n");
        reference_materials.push_str(&base_content);
        info!("Loaded base page content from database");
    }

    // Include all parent paths in the hierarchy
    // e.g., for /articles/2023/why-i-write, include /articles/2023 and /articles
    let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if path_segments.len() > 1 {
        // Build parent paths from top to bottom (excluding the current path)
        for i in 1..path_segments.len() {
            let parent_path = format!("/{}", path_segments[..i].join("/"));

            if let Ok(Some(parent_content)) = state.db.get(&parent_path, "").await {
                if !reference_materials.is_empty() {
                    reference_materials.push_str("\n\n");
                } else {
                    reference_materials.push_str("## Reference materials\n\n");
                }
                reference_materials.push_str("### ");
                reference_materials.push_str(&parent_path);
                reference_materials.push_str(" (parent)\n\n");
                reference_materials.push_str(&parent_content);
                info!(parent_path = %parent_path, "Loaded parent path content from database");
            }
        }
    }

    // For POST requests, include the request body in reference materials
    if method == Method::POST && !body_str.is_empty() {
        if !reference_materials.is_empty() {
            reference_materials.push_str("\n\n");
        }
        reference_materials.push_str("## Request Body\n\n");
        reference_materials.push_str(body_str);
    }

    reference_materials
}

/// Checks the database for GET requests and returns stored content if available.
async fn check_cache(
    state: &AppState,
    method: &Method,
    path: &str,
    uri: &Uri,
    content_type_header: &str,
) -> Result<Option<Response>, Response> {
    if method != Method::GET {
        return Ok(None);
    }

    let query = uri.query().unwrap_or("");

    match state.db.get(path, query).await {
        Ok(Some(content)) => {
            info!(query = %query, "Database hit");
            Ok(Some(
                ([("Content-Type", content_type_header)], content).into_response(),
            ))
        }
        Ok(None) => {
            info!(query = %query, "Database miss");
            Ok(None)
        }
        Err(e) => {
            info!(query = %query, error = %e, "Database read error");
            // Continue to generation if database read fails
            Ok(None)
        }
    }
}

/// Checks if the request is already in-flight and returns an error response if so.
async fn check_in_flight(
    state: &AppState,
    method: &Method,
    path_and_query: &str,
) -> Result<(), Response> {
    if method != Method::GET {
        return Ok(());
    }

    let is_in_flight = {
        let in_flight = state.in_flight.read().await;
        in_flight.contains(path_and_query)
    };

    if is_in_flight {
        info!("Request already in-flight, returning 503 Service Unavailable");
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            [("Retry-After", "1")],
            "Content generation in progress. Please retry shortly.",
        )
            .into_response());
    }

    Ok(())
}

/// Registers the request as in-flight for GET requests.
/// Returns true if successfully registered, false if not applicable (non-GET requests).
async fn register_in_flight(
    state: &AppState,
    method: &Method,
    path_and_query: &str,
) -> Result<bool, Response> {
    if method != Method::GET {
        return Ok(false);
    }

    let mut in_flight = state.in_flight.write().await;

    // Double-check that another request didn't register while we were acquiring the write lock
    if in_flight.contains(path_and_query) {
        drop(in_flight); // Release the write lock

        info!("Request became in-flight while acquiring lock, returning 503 Service Unavailable");
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            [("Retry-After", "1")],
            "Content generation in progress. Please retry shortly.",
        )
            .into_response());
    }

    in_flight.insert(path_and_query.to_string());
    info!("Registered as in-flight");
    Ok(true)
}

/// Parameters for content generation
struct GenerateParams<'a> {
    content_type: &'a crate::config::ContentTypeConfig,
    mime_type: &'a str,
    path_and_query: &'a str,
    referer: &'a str,
    reference_materials: &'a str,
    method: &'a Method,
    path: &'a str,
    uri: &'a Uri,
}

/// Generates content using the OpenAI API and stores it in the database for GET requests.
async fn generate_content(
    state: &AppState,
    client: &OpenRouterClient,
    params: GenerateParams<'_>,
) -> Response {
    let env = create_template_env();

    // Build user prompt with error handling
    let mut prompt_builder = params
        .content_type
        .user_prompt_builder(params.path_and_query.to_string());

    if !params.referer.is_empty() {
        prompt_builder = prompt_builder.headers(params.referer.to_string());
    }

    if !params.reference_materials.is_empty() {
        prompt_builder = prompt_builder.reference_materials(params.reference_materials.to_string());
    }

    let user_prompt = match prompt_builder.build() {
        Ok(prompt) => prompt,
        Err(e) => {
            info!(error = %e, "Failed to render user prompt template");
            let error_html = env
                .get_template("build_request_error")
                .and_then(|tmpl| tmpl.render(minijinja::context! { error => e.to_string() }))
                .unwrap_or_else(|_| {
                    format!(
                        "<h1>Error generating page</h1><p>Failed to render template: {}</p>",
                        e
                    )
                });
            return axum::response::Html(error_html).into_response();
        }
    };

    let request = ChatCompletionRequest {
        model: params.content_type.model.clone(),
        messages: vec![
            Message {
                role: MessageRole::System,
                content: params.content_type.system_prompt.clone(),
            },
            Message {
                role: MessageRole::User,
                content: user_prompt,
            },
        ],
        provider: Some(ProviderPrefs {
            sort: ProviderSort::Latency,
        }),
    };

    let start = std::time::Instant::now();
    info!(
        model = %params.content_type.model,
        content_type = %params.mime_type,
        "Calling API"
    );

    match client.chat_completion(request).await {
        Ok(response) => {
            let duration = start.elapsed();
            let content = response
                .choices
                .first()
                .map(|choice| choice.message.content.clone())
                .unwrap_or_default();

            info!(
                duration_secs = %format!("{:.2}", duration.as_secs_f64()),
                bytes = %content.len(),
                content_type = %params.mime_type,
                "API responded"
            );

            // Save to database only for GET requests
            if params.method == Method::GET {
                let query = params.uri.query().unwrap_or("");

                match state.db.set(params.path, query, &content).await {
                    Ok(_) => {
                        info!(query = %query, "Stored generation in database");
                    }
                    Err(e) => {
                        info!(query = %query, error = %e, "Failed to store generation in database");
                        // Continue serving the response even if storing fails
                    }
                }
            }

            (
                [(
                    "Content-Type",
                    params.content_type.content_type_header.as_str(),
                )],
                content,
            )
                .into_response()
        }
        Err(e) => {
            let duration = start.elapsed();

            // Log error with full chain of causes
            let error_chain: Vec<String> = e.chain().map(|e| e.to_string()).collect();
            let error_msg = error_chain.join("\n  caused by: ");

            warn!(
                duration_secs = %format!("{:.2}", duration.as_secs_f64()),
                error = %error_msg,
                "API error"
            );

            let error_html = env
                .get_template("api_error")
                .and_then(|tmpl| tmpl.render(minijinja::context! { error => e.to_string() }))
                .unwrap_or_else(|_| format!("<h1>Error generating page</h1><p>{}</p>", e));
            axum::response::Html(error_html).into_response()
        }
    }
}

#[tracing::instrument(skip(state, req), fields(req = %format!("{} {}", req.method(), req.uri().path())))]
pub async fn handle(State(state): State<Arc<AppState>>, req: Request) -> impl IntoResponse {
    let uri = req.uri().clone();
    let method = req.method().clone();
    let headers = req.headers().clone();

    let path_and_query = uri.path_and_query().unwrap().as_str();
    let path = normalize_path(uri.path());

    info!("Request received");

    // Extract referer header if present
    let referer = headers
        .get("referer")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Extract request body
    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            info!(error = %e, "Failed to read request body");
            return (StatusCode::BAD_REQUEST, "Failed to read request body").into_response();
        }
    };

    let body_str = String::from_utf8_lossy(&body_bytes).to_string();

    // Determine content type based on method, Accept header, and path
    let (mime_type, content_type) = match determine_content_type(&method, &headers, path, &state) {
        Ok(result) => result,
        Err(response) => return *response,
    };

    // Build reference materials from database-stored referer, base page, parent paths, and request body
    let reference_materials =
        build_reference_materials(&state, referer, &uri, path, &method, &body_str).await;

    // Check database for GET requests
    if let Some(cached_response) = check_cache(
        &state,
        &method,
        path,
        &uri,
        &content_type.content_type_header,
    )
    .await
    .unwrap_or(None)
    {
        return cached_response;
    }

    // Check if this path is already being generated by another request
    if let Err(response) = check_in_flight(&state, &method, path_and_query).await {
        return response;
    }

    // For GET requests, register this request as in-flight
    let is_registered = match register_in_flight(&state, &method, path_and_query).await {
        Ok(registered) => registered,
        Err(response) => return response,
    };

    // Generate content using the shared OpenRouter client
    let result = generate_content(
        &state,
        &state.openrouter_client,
        GenerateParams {
            content_type,
            mime_type,
            path_and_query,
            referer,
            reference_materials: &reference_materials,
            method: &method,
            path,
            uri: &uri,
        },
    )
    .await;

    // Clean up in-flight tracking
    if is_registered {
        let mut in_flight = state.in_flight.write().await;
        in_flight.remove(path_and_query);
        info!("Removed from in-flight tracking");
    }

    result
}
