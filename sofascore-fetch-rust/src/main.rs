use futures_util::StreamExt;
use std::{
    env,
    error::Error,
    fmt,
    io::{self, Write},
    time::Duration,
};
use wreq::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, REFERER, USER_AGENT};
use wreq::{Client, redirect};
use wreq_util::{Emulation, Platform, Profile};

const TIMEOUT: Duration = Duration::from_secs(15);
const PREVIEW_LIMIT: usize = 200;
const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36";

#[derive(Debug)]
struct FetchError(String);
impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for FetchError {}

fn validate_url(value: &str) -> Result<url::Url, FetchError> {
    let url = url::Url::parse(value)
        .map_err(|_| FetchError("expected a SofaScore HTTPS API URL".into()))?;
    let host_allowed = matches!(
        url.host_str(),
        Some("www.sofascore.com" | "sofascore.com" | "api.sofascore.com")
    );
    if url.scheme() != "https" || !host_allowed || !url.path().starts_with("/api/v1/") {
        return Err(FetchError("expected a SofaScore HTTPS API URL".into()));
    }
    Ok(url)
}

fn safe_redirect(uri: &wreq::Uri) -> bool {
    let raw = uri.to_string();
    validate_url(&raw).is_ok()
}

fn request_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert(
        REFERER,
        HeaderValue::from_static("https://www.sofascore.com/"),
    );
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
    headers.insert(
        "x-requested-with",
        HeaderValue::from_static("XMLHttpRequest"),
    );
    headers
}

fn client() -> Result<Client, FetchError> {
    let emulation = Emulation::builder()
        .profile(Profile::Chrome149)
        .platform(Platform::MacOS)
        .headers(false)
        .build();
    Client::builder()
        .emulation(emulation)
        .default_headers(request_headers())
        .timeout(TIMEOUT)
        .redirect(redirect::Policy::custom(|attempt| {
            if attempt.previous.len() >= 10 {
                attempt.error("too many redirects")
            } else if safe_redirect(&attempt.uri) {
                attempt.follow()
            } else {
                attempt.error("redirect target is outside allowed SofaScore API URLs")
            }
        }))
        .build()
        .map_err(|e| FetchError(format!("create HTTP client: {e}")))
}

async fn fetch_with_client(client: &Client, url: url::Url) -> Result<Vec<u8>, FetchError> {
    let response = client
        .get(url.as_str())
        .send()
        .await
        .map_err(|e| FetchError(format!("request failed: {e}")))?;
    let status = response.status();
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    if !status.is_success() {
        while body.len() < PREVIEW_LIMIT {
            match stream.next().await {
                Some(Ok(chunk)) => {
                    body.extend_from_slice(&chunk[..chunk.len().min(PREVIEW_LIMIT - body.len())])
                }
                Some(Err(e)) => return Err(FetchError(format!("request failed: {e}"))),
                None => break,
            }
        }
        let preview = String::from_utf8_lossy(&body).trim().to_owned();
        return Err(FetchError(format!("HTTP {}: {}", status.as_u16(), preview)));
    }
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| FetchError(format!("read response: {e}")))?;
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

async fn fetch_with_timeout(
    client: &Client,
    url: url::Url,
    timeout: Duration,
) -> Result<Vec<u8>, FetchError> {
    tokio::time::timeout(timeout, fetch_with_client(client, url))
        .await
        .map_err(|_| FetchError("request failed: timed out".into()))?
}

async fn fetch(url: url::Url) -> Result<Vec<u8>, FetchError> {
    let client = client()?;
    fetch_with_timeout(&client, url, TIMEOUT).await
}

fn run() -> Result<(), FetchError> {
    let mut args = env::args();
    let _program = args.next();
    let target = match (args.next(), args.next()) {
        (Some(target), None) => target,
        _ => {
            return Err(FetchError(
                "usage: sofascore_fetch https://www.sofascore.com/api/v1/...".into(),
            ));
        }
    };
    let url = validate_url(&target)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| FetchError(format!("create runtime: {e}")))?;
    let body = runtime.block_on(fetch(url))?;
    io::stdout()
        .write_all(&body)
        .map_err(|e| FetchError(format!("write response: {e}")))?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_https_api_urls_on_allowed_hosts() {
        assert!(validate_url("https://www.sofascore.com/api/v1/event/1").is_ok());
        assert!(validate_url("https://api.sofascore.com/api/v1/event/1").is_ok());
        for invalid in [
            "http://www.sofascore.com/api/v1/event/1",
            "https://evil.example/api/v1/event/1",
            "https://sofascore.com/other/path",
            "https://sofascore.com.evil.example/api/v1/event/1",
        ] {
            assert!(validate_url(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn redirect_targets_obey_the_same_restrictions() {
        assert!(safe_redirect(
            &"https://www.sofascore.com/api/v1/team/2".parse().unwrap()
        ));
        assert!(!safe_redirect(
            &"https://example.com/api/v1/team/2".parse().unwrap()
        ));
        assert!(!safe_redirect(
            &"https://sofascore.com/login".parse().unwrap()
        ));
    }

    #[test]
    fn request_headers_match_the_legacy_cli_contract() {
        let h = request_headers();
        assert_eq!(h.len(), 5);
        assert_eq!(h[ACCEPT], "application/json");
        assert_eq!(h[ACCEPT_LANGUAGE], "en-US,en;q=0.9");
        assert_eq!(h[REFERER], "https://www.sofascore.com/");
        assert_eq!(h[USER_AGENT], USER_AGENT_VALUE);
        assert_eq!(h["x-requested-with"], "XMLHttpRequest");
    }

    fn serve_once(response: Vec<u8>) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            stream.write_all(&response).unwrap();
            String::from_utf8(request).unwrap()
        });
        (format!("http://{address}/api/v1/test"), thread)
    }

    #[test]
    fn local_transport_preserves_success_body_bytes_and_sends_only_legacy_headers() {
        let (url, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\n\x00\xffhello"
                .to_vec(),
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let body = runtime
            .block_on(fetch_with_client(
                &client().unwrap(),
                url::Url::parse(&url).unwrap(),
            ))
            .unwrap();
        assert_eq!(body, b"\x00\xffhello");
        let request = server.join().unwrap().to_ascii_lowercase();
        assert!(request.contains("accept: application/json\r\n"));
        assert!(request.contains("accept-language: en-us,en;q=0.9\r\n"));
        assert!(request.contains("referer: https://www.sofascore.com/\r\n"));
        assert!(request.contains(&format!(
            "user-agent: {}\r\n",
            USER_AGENT_VALUE.to_ascii_lowercase()
        )));
        assert!(request.contains("x-requested-with: xmlhttprequest\r\n"));
        assert!(!request.contains("accept-encoding:"));
        assert!(!request.contains("sec-fetch-"));
        assert!(!request.contains("sec-ch-"));
    }

    #[test]
    fn local_transport_limits_non_success_preview_to_200_bytes() {
        let payload = vec![b'x'; 500];
        let (url, server) = serve_once(
            [b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 500\r\nConnection: close\r\n\r\n".as_slice(), payload.as_slice()].concat(),
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = runtime
            .block_on(fetch_with_client(
                &client().unwrap(),
                url::Url::parse(&url).unwrap(),
            ))
            .unwrap_err();
        assert_eq!(error.to_string().len(), "HTTP 503: ".len() + PREVIEW_LIMIT);
        assert!(error.to_string().ends_with(&"x".repeat(PREVIEW_LIMIT)));
        server.join().unwrap();
    }

    #[test]
    fn local_transport_rejects_redirects_outside_the_allowed_https_scope() {
        let (url, server) = serve_once(
            b"HTTP/1.1 302 Found\r\nLocation: /api/v1/other\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = runtime
            .block_on(fetch_with_client(
                &client().unwrap(),
                url::Url::parse(&url).unwrap(),
            ))
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("outside allowed SofaScore API URLs")
        );
        server.join().unwrap();
    }

    #[test]
    fn total_timeout_includes_a_stalled_response_body() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n")
                .unwrap();
            std::thread::sleep(Duration::from_millis(300));
            let _ = stream.write_all(b"x");
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let url = url::Url::parse(&format!("http://{address}/api/v1/test")).unwrap();
        let http_client = client().unwrap();
        let result = runtime.block_on(fetch_with_timeout(
            &http_client,
            url,
            Duration::from_millis(75),
        ));
        assert!(
            result.is_err(),
            "stalled response body outlived total timeout"
        );
        server.join().unwrap();
    }
}
