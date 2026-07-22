//! Smoke test for the OTLP log export path.
//!
//! Verifies that `jarvy bootstrap` with `JARVY_TELEMETRY_SMOKE=1`
//! sends at least one request to `/v1/logs` on the configured OTLP
//! endpoint. The actual log payload shape isn't asserted — that's
//! covered by unit tests on `tool_failed_with_kind` etc. This test
//! just smoke-checks the wire pipeline.
//!
//! Flakiness-resistant design (previously flaked roughly 1-in-30
//! runs due to a hardcoded port + 10s deadline):
//! - Binds a random ephemeral port (`port 0`) rather than 4318.
//!   Concurrent test runs (or any other OTLP collector on the host)
//!   no longer race for the port.
//! - `JARVY_OTLP_ENDPOINT` is passed to the CLI so both the OTEL
//!   log exporter AND `analytics::send_otlp_smoke_probe` target the
//!   test's port (the latter was previously hardcoded to 4318).
//! - `#[serial]` keeps this test from racing other env-var-mutating
//!   tests in the same binary.
//! - Server accept loop polls every 10ms with a 30s wall-clock
//!   deadline (was 10s — CLI cold-start on a busy CI runner can
//!   easily exceed that).
//! - Server is `set_nonblocking(true)` and the accept thread is
//!   spawned BEFORE the CLI so the listen queue is ready when the
//!   probe lands.

use assert_cmd::prelude::*;
use serial_test::serial;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

fn handle_client(mut stream: TcpStream, logs_seen: &Arc<AtomicBool>) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    let headers_end;
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => return,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if let Some(i) = find_bytes(&buf, b"\r\n\r\n") {
                    headers_end = i + 4;
                    break;
                }
                if buf.len() > 1024 * 1024 {
                    return;
                }
            }
            Err(_) => return,
        }
    }

    let headers = match std::str::from_utf8(&buf[..headers_end]) {
        Ok(h) => h,
        Err(_) => return,
    };
    let mut lines = headers.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let _method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    if path.contains("/v1/logs") {
        logs_seen.store(true, Ordering::SeqCst);
    }

    let mut content_len: usize = 0;
    for line in lines {
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            if let Ok(v) = rest.trim().parse::<usize>() {
                content_len = v;
            }
        } else if let Some(rest) = line.strip_prefix("content-length:")
            && let Ok(v) = rest.trim().parse::<usize>()
        {
            content_len = v;
        }
    }

    let already = buf.len().saturating_sub(headers_end);
    let to_read = content_len.saturating_sub(already);
    let mut remaining = to_read;
    while remaining > 0 {
        let n: usize = stream.read(&mut tmp).unwrap_or_default();
        if n == 0 {
            break;
        }
        remaining = remaining.saturating_sub(n);
    }

    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let _ = stream.flush();
}

#[test]
#[serial]
fn telemetry_smoke_error_logs_only() -> Result<(), Box<dyn std::error::Error>> {
    // Bind a random ephemeral port instead of the hardcoded 4318.
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;

    let endpoint = format!("http://127.0.0.1:{}", port);
    let logs_seen = Arc::new(AtomicBool::new(false));
    let logs_seen_srv = Arc::clone(&logs_seen);
    let server_done = Arc::new(AtomicBool::new(false));
    let server_done_srv = Arc::clone(&server_done);

    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    handle_client(stream, &logs_seen_srv);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() > deadline || server_done_srv.load(Ordering::SeqCst) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
            if logs_seen_srv.load(Ordering::SeqCst) {
                break;
            }
        }
    });

    // Run the CLI pointed at our ephemeral port.
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    let assert = cmd
        .env("JARVY_TEST_MODE", "1")
        .env("JARVY_TELEMETRY_SMOKE", "1")
        .env("JARVY_OTLP_ENDPOINT", &endpoint)
        .arg("bootstrap")
        .assert();
    assert.success();

    // Wait up to 30s for the OTEL exporter / smoke probe to land. The
    // CLI's own 800ms grace period inside the smoke path runs before
    // the assert returns, but the kernel may schedule the server
    // thread late on a busy CI runner.
    let deadline = Instant::now() + Duration::from_secs(30);
    while !logs_seen.load(Ordering::SeqCst) {
        if Instant::now() > deadline {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }

    server_done.store(true, Ordering::SeqCst);
    let _ = server.join();

    assert!(
        logs_seen.load(Ordering::SeqCst),
        "no request to /v1/logs observed on {} within 30s",
        endpoint
    );

    Ok(())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}
