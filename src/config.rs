use std::collections::HashMap;

use anyhow::Result;
use minijinja::Environment;
use serde::Deserialize;

/// Configuration for a single content type
#[derive(Debug, Clone, Deserialize)]
pub struct ContentTypeConfig {
    pub model: String,
    pub system_prompt: String,
    pub content_type_header: String,
    pub extensions: Vec<String>,
}

impl ContentTypeConfig {
    pub fn user_prompt_builder(&self, path: String) -> UserPromptBuilder {
        UserPromptBuilder {
            path,
            headers: None,
            reference_materials: None,
        }
    }
}

/// Builder for constructing user prompts
pub struct UserPromptBuilder {
    path: String,
    headers: Option<String>,
    reference_materials: Option<String>,
}

impl UserPromptBuilder {
    pub fn headers(mut self, headers: String) -> Self {
        self.headers = Some(headers);
        self
    }

    pub fn reference_materials(mut self, reference_materials: String) -> Self {
        self.reference_materials = Some(reference_materials);
        self
    }

    pub fn build(self) -> Result<String> {
        const USER_PROMPT_TEMPLATE: &str = r#"Generate content for path: {{ path }}

The following materials are context-only. They are **not part of the output**.
Use them only to stay consistent with style or data conventions.

Headers: {{ headers }}
Reference materials: {{ reference_materials }}"#;

        let mut env = Environment::new();
        env.add_template("user_prompt", USER_PROMPT_TEMPLATE)?;

        let template = env.get_template("user_prompt")?;
        let prompt = template.render(minijinja::context! {
            path => self.path,
            headers => self.headers.unwrap_or_else(|| "none".to_string()),
            reference_materials => self.reference_materials.unwrap_or_else(|| "none".to_string()),
        })?;

        Ok(prompt)
    }
}

/// Root configuration structure
#[derive(Debug, Deserialize)]
pub struct WebSimConfig {
    pub content_types: HashMap<String, ContentTypeConfig>,
}
