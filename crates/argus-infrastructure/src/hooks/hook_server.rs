use std::str::FromStr;
use std::thread;

use argus_application::ports::HookCallbackPort;
use argus_domain::SessionId;
use tiny_http::{Method, Response, Server};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEventKind {
    PromptSubmitted,
    Stopped,
    Notification,
}

/// A tiny local HTTP server bound to `127.0.0.1` on an OS-assigned ephemeral
/// port, started once at app startup. Each Session's spawned `claude`
/// process is configured (via `--settings`, see `CreateSessionUseCase`) to
/// GET `<callback_url>?sessionId=<uuid>&event=<prompt_submitted|stop>` from
/// its `UserPromptSubmit`/`Stop` hooks; this server parses that and invokes
/// `on_event`, which the composition root folds into a Session Runtime
/// Status update. GET (not POST+JSON body) specifically because the hook
/// `command` string is interpreted by the OS shell (`cmd.exe` on Windows,
/// `/bin/sh` elsewhere) — a query string needs no shell-quoting, whereas an
/// embedded JSON body would need different escaping per platform.
pub struct HookServer {
    port: u16,
}

impl HookServer {
    pub fn start(
        on_event: impl Fn(SessionId, HookEventKind) + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let server = Server::http("127.0.0.1:0").map_err(std::io::Error::other)?;
        let port = server
            .server_addr()
            .to_ip()
            .expect("HookServer is always bound to a concrete IPv4/IPv6 address, never a unix socket")
            .port();

        thread::spawn(move || {
            for request in server.incoming_requests() {
                let event = (request.method() == &Method::Get)
                    .then(|| parse_hook_request(request.url()))
                    .flatten();
                if let Some((session_id, kind)) = event {
                    on_event(session_id, kind);
                }
                let _ = request.respond(Response::empty(204));
            }
        });

        Ok(Self { port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl HookCallbackPort for HookServer {
    fn callback_url(&self) -> String {
        format!("http://127.0.0.1:{}/hook", self.port)
    }
}

/// Parses `/hook?sessionId=<uuid>&event=<prompt_submitted|stop>` out of a
/// request path+query string. No query-string library pulled in for two
/// known, simple, non-URL-encoded fields.
fn parse_hook_request(url: &str) -> Option<(SessionId, HookEventKind)> {
    let (path, query) = url.split_once('?')?;
    if path != "/hook" {
        return None;
    }

    let mut session_id = None;
    let mut event = None;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=')?;
        match key {
            "sessionId" => session_id = uuid::Uuid::from_str(value).ok().map(SessionId::from),
            "event" => {
                event = match value {
                    "prompt_submitted" => Some(HookEventKind::PromptSubmitted),
                    "stop" => Some(HookEventKind::Stopped),
                    "notification" => Some(HookEventKind::Notification),
                    _ => None,
                }
            }
            _ => {}
        }
    }

    Some((session_id?, event?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_well_formed_hook_request() {
        let id = SessionId::new();
        let url = format!("/hook?sessionId={id}&event=prompt_submitted");
        let (parsed_id, kind) = parse_hook_request(&url).unwrap();
        assert_eq!(parsed_id, id);
        assert_eq!(kind, HookEventKind::PromptSubmitted);
    }

    #[test]
    fn parses_a_notification_event() {
        let id = SessionId::new();
        let url = format!("/hook?sessionId={id}&event=notification");
        let (parsed_id, kind) = parse_hook_request(&url).unwrap();
        assert_eq!(parsed_id, id);
        assert_eq!(kind, HookEventKind::Notification);
    }

    #[test]
    fn rejects_wrong_path() {
        assert!(parse_hook_request("/not-hook?sessionId=x&event=stop").is_none());
    }

    #[test]
    fn rejects_missing_fields() {
        assert!(parse_hook_request("/hook?event=stop").is_none());
        assert!(parse_hook_request("/hook?sessionId=not-a-uuid&event=stop").is_none());
    }
}
