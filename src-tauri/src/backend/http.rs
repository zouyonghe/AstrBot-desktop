use std::{
    io::{ErrorKind, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use url::Url;

use crate::{backend::http_response, BackendState, GRACEFUL_RESTART_START_TIME_TIMEOUT_MS};

impl BackendState {
    pub(crate) fn ping_backend(&self, timeout_ms: u64) -> bool {
        let parsed = match Url::parse(&self.backend_url) {
            Ok(url) => url,
            Err(_) => return false,
        };
        let host = match parsed.host_str() {
            Some(host) => host.to_string(),
            None => return false,
        };
        let port = parsed.port_or_known_default().unwrap_or(80);
        let timeout = Duration::from_millis(timeout_ms.max(50));

        let addrs = match (host.as_str(), port).to_socket_addrs() {
            Ok(addrs) => addrs.collect::<Vec<_>>(),
            Err(_) => return false,
        };
        addrs
            .iter()
            .any(|address| TcpStream::connect_timeout(address, timeout).is_ok())
    }

    pub(crate) fn request_backend_response_bytes(
        &self,
        method: &str,
        api_path: &str,
        timeout_ms: u64,
        body: Option<&str>,
        auth_token: Option<&str>,
    ) -> Option<Vec<u8>> {
        let base = Url::parse(&self.backend_url).ok()?;
        let request_url = base.join(api_path).ok()?;
        if request_url.scheme() != "http" {
            return None;
        }

        let host = request_url.host_str()?;
        let port = request_url.port_or_known_default().unwrap_or(80);
        let timeout = Duration::from_millis(timeout_ms.max(50));
        let addrs = (host, port).to_socket_addrs().ok()?;
        let mut stream = addrs
            .into_iter()
            .find_map(|address| TcpStream::connect_timeout(&address, timeout).ok())?;
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));

        let mut request_target = request_url.path().to_string();
        if let Some(query) = request_url.query() {
            request_target.push('?');
            request_target.push_str(query);
        }
        if request_target.is_empty() {
            request_target = "/".to_string();
        }

        let payload = body.unwrap_or("");
        let authorization_header = auth_token
            .and_then(sanitize_authorization_token)
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        let request = format!(
            "{method} {request_target} HTTP/1.1\r\n\
Host: {host}\r\n\
Accept: application/json\r\n\
Accept-Encoding: identity\r\n\
Connection: close\r\n\
{authorization_header}\
Content-Type: application/json\r\n\
Content-Length: {}\r\n\
\r\n\
{}",
            payload.len(),
            payload
        );
        if stream.write_all(request.as_bytes()).is_err() {
            return None;
        }

        read_http_response_bytes(&mut stream)
    }

    pub(crate) fn request_backend_with<T, F>(
        &self,
        method: &str,
        api_path: &str,
        timeout_ms: u64,
        body: Option<&str>,
        auth_token: Option<&str>,
        parse: F,
    ) -> Option<T>
    where
        F: FnOnce(&[u8]) -> Option<T>,
    {
        let response =
            self.request_backend_response_bytes(method, api_path, timeout_ms, body, auth_token)?;
        parse(&response)
    }

    pub(crate) fn request_backend_json(
        &self,
        method: &str,
        api_path: &str,
        timeout_ms: u64,
        body: Option<&str>,
        auth_token: Option<&str>,
    ) -> Option<serde_json::Value> {
        self.request_backend_with(
            method,
            api_path,
            timeout_ms,
            body,
            auth_token,
            http_response::parse_http_json_response,
        )
    }

    pub(crate) fn request_backend_status_code(
        &self,
        method: &str,
        api_path: &str,
        timeout_ms: u64,
        body: Option<&str>,
        auth_token: Option<&str>,
    ) -> Option<u16> {
        self.request_backend_with(
            method,
            api_path,
            timeout_ms,
            body,
            auth_token,
            http_response::parse_http_status_code,
        )
    }

    pub(crate) fn fetch_backend_start_time(&self) -> Option<i64> {
        let payload = self.request_backend_json(
            "GET",
            "/api/stat/start-time",
            GRACEFUL_RESTART_START_TIME_TIMEOUT_MS,
            None,
            None,
        )?;
        http_response::parse_backend_start_time(&payload)
    }
}

fn is_complete_http_response(raw: &[u8]) -> bool {
    let Some(header_end) = raw.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let headers = &raw[..header_end + 4];
    let body = &raw[header_end + 4..];
    let header_text = String::from_utf8_lossy(headers).to_ascii_lowercase();

    if header_text.contains("transfer-encoding: chunked") {
        return body.windows(5).any(|window| window == b"0\r\n\r\n");
    }

    if let Some(content_length) = header_text
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
    {
        return body.len() >= content_length;
    }

    false
}

fn sanitize_authorization_token(token: &str) -> Option<&str> {
    if token.contains('\r') || token.contains('\n') {
        return None;
    }
    let token = token.trim();
    if token.is_empty() {
        return None;
    }
    Some(token)
}

fn read_http_response_bytes<R: Read>(reader: &mut R) -> Option<Vec<u8>> {
    let mut response = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                response.extend_from_slice(&chunk[..read]);
                if is_complete_http_response(&response) {
                    break;
                }
            }
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                if response.is_empty() {
                    return None;
                }
                break;
            }
            Err(_) => return None,
        }
    }

    if response.is_empty() {
        None
    } else {
        Some(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_complete_http_response_respects_content_length() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ntest";
        assert!(is_complete_http_response(raw));
    }

    #[test]
    fn sanitize_authorization_token_rejects_crlf() {
        assert_eq!(sanitize_authorization_token("abc\r\ndef"), None);
    }

    #[test]
    fn sanitize_authorization_token_trims_and_accepts_normal_token() {
        assert_eq!(sanitize_authorization_token("  token  "), Some("token"));
    }

    #[test]
    fn read_http_response_bytes_keeps_partial_data_on_timeout() {
        struct TimeoutReader {
            chunks: Vec<Result<&'static [u8], std::io::ErrorKind>>,
            index: usize,
        }

        impl Read for TimeoutReader {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.index >= self.chunks.len() {
                    return Ok(0);
                }
                let chunk = self.chunks[self.index];
                self.index += 1;
                match chunk {
                    Ok(bytes) => {
                        let n = bytes.len().min(buf.len());
                        buf[..n].copy_from_slice(&bytes[..n]);
                        Ok(n)
                    }
                    Err(kind) => Err(std::io::Error::from(kind)),
                }
            }
        }

        let mut reader = TimeoutReader {
            chunks: vec![
                Ok(b"HTTP/1.1 200 OK\r\n"),
                Err(std::io::ErrorKind::TimedOut),
            ],
            index: 0,
        };
        let bytes = read_http_response_bytes(&mut reader).expect("expected partial response");
        assert_eq!(bytes, b"HTTP/1.1 200 OK\r\n");
    }
}
