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

    println!("HTTP CLIENT: Sent request to {}:\n{}", host_port, request_str);

    let response = read_http_response(&mut stream).await?;

    println!("HTTP CLIENT: Received raw response from {}:\n{}", host_port, response);

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

    let response = read_http_response(&mut stream).await?;

    if let Some(pos) = response.find("\r\n\r\n") {
        Ok(response[pos + 4..].to_string())
    } else {
        Err("Invalid HTTP response format".to_string())
    }
}

async fn read_http_response(stream: &mut tokio::net::TcpStream) -> Result<String, String> {
    use tokio::time::timeout;
    use std::time::Duration;

    let read_fut = async {
        let mut buf = Vec::new();
        let mut temp = [0u8; 1024];
        let mut header_end_pos = None;

        // 1. Read until we find the end of the headers (\r\n\r\n)
        loop {
            let n = stream.read(&mut temp).await.map_err(|e| format!("Read error: {}", e))?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&temp[..n]);

            // Search for \r\n\r\n
            if let Some(pos) = find_subsequence(&buf, b"\r\n\r\n") {
                header_end_pos = Some(pos);
                break;
            }
        }

        let header_end = match header_end_pos {
            Some(pos) => pos,
            None => {
                return Ok(String::from_utf8_lossy(&buf).into_owned());
            }
        };

        // Parse headers to find Content-Length
        let headers_part = String::from_utf8_lossy(&buf[..header_end]);
        let mut content_length = None;
        for line in headers_part.lines() {
            let lower = line.to_lowercase();
            if lower.starts_with("content-length:") {
                if let Some(val_str) = line.split(':').nth(1) {
                    if let Ok(len) = val_str.trim().parse::<usize>() {
                        content_length = Some(len);
                    }
                }
            }
        }

        let body_start = header_end + 4;
        let mut response_str = String::from_utf8_lossy(&buf).into_owned();

        if let Some(len) = content_length {
            let current_body_len = buf.len() - body_start;
            if current_body_len < len {
                let remaining = len - current_body_len;
                let mut remaining_buf = vec![0u8; remaining];
                stream.read_exact(&mut remaining_buf).await.map_err(|e| format!("Read remaining body error: {}", e))?;
                let remaining_str = String::from_utf8_lossy(&remaining_buf);
                response_str.push_str(&remaining_str);
            }
        } else {
            // Read to end
            let mut remaining_buf = Vec::new();
            stream.read_to_end(&mut remaining_buf).await.map_err(|e| format!("Read to end error: {}", e))?;
            let remaining_str = String::from_utf8_lossy(&remaining_buf);
            response_str.push_str(&remaining_str);
        }

        Ok(response_str)
    };

    timeout(Duration::from_secs(5), read_fut)
        .await
        .map_err(|_| "HTTP read timed out".to_string())?
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}
