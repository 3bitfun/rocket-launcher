use super::{microsoft, pkce::AuthorizationAttempt};
use crate::errors::{AppError, Result};
use std::time::Duration;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}};

pub async fn sign_in(client_id: &str) -> Result<microsoft::MicrosoftTokens> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await
        .map_err(|_| AppError::new("AUTH_CALLBACK", "Could not open the local sign-in callback."))?;
    let port = listener.local_addr().map_err(|_| AppError::new("AUTH_CALLBACK", "Could not find the local callback port."))?.port();
    let host = format!("localhost:{port}");
    let redirect = format!("http://{host}/callback");
    let expected = reqwest::Url::parse(&redirect).expect("constructed loopback URL");
    let attempt = AuthorizationAttempt::new();
    open(microsoft::authorization_url(client_id, &redirect, &attempt)?)?;
    let code = tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            let (mut stream, _) = listener.accept().await.map_err(|_| AppError::new("AUTH_CALLBACK", "The sign-in callback stopped listening."))?;
            let code = tokio::time::timeout(Duration::from_secs(3), read_code(&mut stream, &host, &expected, &attempt)).await.ok().flatten();
            let reply = if code.is_some() { "200 OK\r\n\r\nReturn to Rocket Launcher to finish signing in." }
                else { "400 Bad Request\r\n\r\nInvalid sign-in callback." };
            let (status, body) = reply.split_once("\r\n\r\n").unwrap();
            let response = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len());
            let _ = tokio::time::timeout(Duration::from_secs(2), stream.write_all(response.as_bytes())).await;
            if let Some(code) = code { return Ok::<_, AppError>(code); }
        }
    }).await.map_err(|_| AppError::new("AUTH_TIMEOUT", "Microsoft sign-in timed out. Start sign-in again."))??;
    drop(listener);
    microsoft::redeem(client_id, &redirect, &code, &attempt).await
}

async fn read_code(stream: &mut TcpStream, host: &str, expected: &reqwest::Url, attempt: &AuthorizationAttempt) -> Option<String> {
    let mut buf = Vec::new();
    let mut chunk = [0; 1024];
    while buf.len() < 16384 {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 { return None; }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(end) = buf.windows(4).position(|v| v == b"\r\n\r\n") {
            let head = std::str::from_utf8(&buf[..end]).ok()?;
            let mut lines = head.split("\r\n");
            let mut request = lines.next()?.split_ascii_whitespace();
            if request.next()? != "GET" { return None; }
            let target = request.next()?;
            if !target.starts_with("/callback?") || !matches!(request.next()?, "HTTP/1.0" | "HTTP/1.1") || request.next().is_some() { return None; }
            let mut hosts = lines.filter_map(|line| line.split_once(':')).filter(|(name, _)| name.eq_ignore_ascii_case("host"));
            if !hosts.next()?.1.trim().eq_ignore_ascii_case(host) || hosts.next().is_some() { return None; }
            return attempt.callback_code(&format!("http://{host}{target}"), expected).ok();
        }
    }
    None
}

fn open(url: reqwest::Url) -> Result<()> {
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let wide: Vec<u16> = url.as_str().encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let result = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), wide.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) };
    if result as isize <= 32 { return Err(AppError::new("AUTH_BROWSER", "Could not open your default browser for Microsoft sign-in.")); }
    Ok(())
}
