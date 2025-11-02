use crate::config::{ContentTypeConfig, WebSimConfig};

/// Determines the content type based on the Accept header
pub fn determine_from_accept<'a>(
    accept_header: Option<&str>,
    config: &'a WebSimConfig,
) -> Option<(&'a String, &'a ContentTypeConfig)> {
    let accept = accept_header?;

    // Check for specific content types in the Accept header
    // Try to find a matching mime type in the config
    for (mime_type, content_config) in &config.content_types {
        if accept.contains(mime_type.as_str()) {
            return Some((mime_type, content_config));
        }
    }

    None
}

/// Determines the content type based on the request path
pub fn determine_from_path<'a>(
    path: &str,
    config: &'a WebSimConfig,
) -> Option<(&'a String, &'a ContentTypeConfig)> {
    // Check if path has a file extension (contains a dot in the last segment)
    let last_segment = path.rsplit('/').next()?;

    if !last_segment.contains('.') {
        // No extension means HTML - look for text/html in config
        return config
            .content_types
            .iter()
            .find(|(mime, _)| mime.as_str() == "text/html");
    }

    // Extract extension and match based on it
    let extension = last_segment.rsplit('.').next()?.to_lowercase();

    // Find content type by extension
    for (mime_type, content_config) in &config.content_types {
        if content_config
            .extensions
            .iter()
            .any(|ext| ext == &extension)
        {
            return Some((mime_type, content_config));
        }
    }

    None
}
