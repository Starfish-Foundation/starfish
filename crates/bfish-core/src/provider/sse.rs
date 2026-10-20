use anyhow::Result;
use futures_util::{Stream, StreamExt};

/// Turn a `text/event-stream` response into a stream of `data:` payloads.
///
/// Providers disagree about blank-line framing (`\n\n` vs `\r\n\r\n`) and send
/// keepalive comments, so this parser is deliberately forgiving.
pub fn sse_data(resp: reqwest::Response) -> impl Stream<Item = Result<String>> + Send + 'static {
    async_stream::try_stream! {
        let mut stream = resp.bytes_stream();
        let mut buf: Vec<u8> = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buf.extend_from_slice(&chunk);

            while let Some((pos, sep_len)) = find_event_end(&buf) {
                let raw = buf[..pos].to_vec();
                buf.drain(..pos + sep_len);
                if let Some(data) = extract_data(&raw) {
                    yield data;
                }
            }
        }

        if !buf.is_empty() {
            if let Some(data) = extract_data(&buf) {
                yield data;
            }
        }
    }
}

fn find_event_end(buf: &[u8]) -> Option<(usize, usize)> {
    let lf = find(buf, b"\n\n").map(|p| (p, 2));
    let crlf = find(buf, b"\r\n\r\n").map(|p| (p, 4));
    match (lf, crlf) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn extract_data(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    let mut lines = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix("data:") {
            lines.push(rest.trim_start().to_string());
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

pub fn truncate_for_error(body: &str) -> String {
    let body = body.trim();
    if body.chars().count() <= 700 {
        body.to_string()
    } else {
        let short: String = body.chars().take(700).collect();
        format!("{short}…")
    }
}
