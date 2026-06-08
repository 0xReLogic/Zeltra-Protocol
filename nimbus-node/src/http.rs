//! HTTP client utilities for making raw TCP-based HTTP requests

use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn post_http(url: &str, body: &str) -> Result<String, String> {
    let clean_url = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let parts: Vec<&str> = clean_url.splitn(2, '/').collect();
    let host_port = parts[0];
    let path = if parts.len() > 1 {
        format!("/{}", parts[1])
    } else {
        "/".to_string()
    };

    let mut stream = tokio::net::TcpStream::connect(host_port)
        .await
        .map_err(|e| format!("Connect failed: {}", e))?;

    let request_str = format!(
        "POST {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n\
         {}",
        path,
        host_port,
        body.len(),
        body
    );

    stream
        .write_all(request_str.as_bytes())
        .await
        .map_err(|e| format!("Write failed: {}", e))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .await
        .map_err(|e| format!("Read failed: {}", e))?;

    if let Some(pos) = response.find("\r\n\r\n") {
        Ok(response[pos + 4..].to_string())
    } else {
        Err("Invalid HTTP response format".to_string())
    }
}

pub async fn get_http_with_headers(url: &str, headers: &[(&str, &str)]) -> Result<String, String> {
    let clean_url = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let parts: Vec<&str> = clean_url.splitn(2, '/').collect();
    let host_port = parts[0];

    // Add port if not specified
    let host_port_with_default = if host_port.contains(':') {
        host_port.to_string()
    } else {
        format!("{}:80", host_port)
    };

    let path = if parts.len() > 1 {
        format!("/{}", parts[1])
    } else {
        "/".to_string()
    };

    let mut stream = tokio::net::TcpStream::connect(&host_port_with_default)
        .await
        .map_err(|e| format!("Connect failed to {}: {}", host_port_with_default, e))?;

    let mut headers_str = String::new();
    for (k, v) in headers {
        headers_str.push_str(&format!("{}: {}\r\n", k, v));
    }

    let request_str = format!(
        "GET {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Connection: close\r\n\
         {}\r\n",
        path, host_port, headers_str
    );

    stream
        .write_all(request_str.as_bytes())
        .await
        .map_err(|e| format!("Write failed: {}", e))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .await
        .map_err(|e| format!("Read failed: {}", e))?;

    if let Some(pos) = response.find("\r\n\r\n") {
        Ok(response[pos + 4..].to_string())
    } else {
        Err("Invalid HTTP response format".to_string())
    }
}
