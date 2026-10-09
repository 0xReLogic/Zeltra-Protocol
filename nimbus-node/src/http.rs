//! HTTP client utilities with TLS, connection pooling, and timeouts

use std::sync::OnceLock;
use std::time::Duration;

/// Global shared HTTP client for outgoing requests (Guardian quorum, Vault KMS, etc.)
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .expect("Failed to initialize global HTTP client")
    })
}

/// Send a JSON POST request with timeout
pub async fn post_http(url: &str, body: &str) -> Result<String, String> {
    let client = get_client();
    let normalized_url = normalize_url(url);

    let res = client
        .post(&normalized_url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| format!("POST request failed to {}: {}", normalized_url, e))?;

    let status = res.status();
    let text = res.text().await.map_err(|e| {
        format!(
            "Failed to read response body from {}: {}",
            normalized_url, e
        )
    })?;

    if status.is_success() {
        Ok(text)
    } else {
        Err(format!(
            "HTTP {} from {}: {}",
            status.as_u16(),
            normalized_url,
            text
        ))
    }
}

/// Send a GET request with custom headers and timeout
pub async fn get_http_with_headers(url: &str, headers: &[(&str, &str)]) -> Result<String, String> {
    let client = get_client();
    let normalized_url = normalize_url(url);

    let mut req = client.get(&normalized_url);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }

    let res = req
        .send()
        .await
        .map_err(|e| format!("GET request failed to {}: {}", normalized_url, e))?;

    let status = res.status();
    let text = res.text().await.map_err(|e| {
        format!(
            "Failed to read response body from {}: {}",
            normalized_url, e
        )
    })?;

    if status.is_success() {
        Ok(text)
    } else {
        Err(format!(
            "HTTP {} from {}: {}",
            status.as_u16(),
            normalized_url,
            text
        ))
    }
}

/// Normalizes URL string to ensure it has http:// or https:// prefix
fn normalize_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{}", trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_url() {
        assert_eq!(
            normalize_url("http://127.0.0.1:8080"),
            "http://127.0.0.1:8080"
        );
        assert_eq!(
            normalize_url("https://vault.internal:8200/v1/secret"),
            "https://vault.internal:8200/v1/secret"
        );
        assert_eq!(
            normalize_url("127.0.0.1:8080/path"),
            "http://127.0.0.1:8080/path"
        );
    }
}
