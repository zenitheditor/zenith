//! Request checks: the per-run token (`Authorization: Bearer` only), the
//! `Host` header (DNS rebinding), and the `Origin` header (cross-site
//! requests).

use std::net::IpAddr;

use zenith_session::adapter::{OsRng, Rng};

use crate::edit::http::{HttpError, HttpRequest};

/// The per-run secret: 32 bytes from the OS RNG, as 64 lowercase hex
/// characters.
#[derive(Clone)]
pub(crate) struct Token(String);

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(..)")
    }
}

impl Token {
    /// A fresh token.
    ///
    /// # Errors
    ///
    /// The OS entropy error.
    pub(crate) fn generate() -> Result<Self, String> {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes).map_err(|e| e.message)?;
        Ok(Self(bytes.iter().map(|b| format!("{b:02x}")).collect()))
    }

    /// A token with a configured `secret`.
    #[cfg(feature = "http")]
    pub(crate) fn from_secret(secret: String) -> Self {
        Self(secret)
    }

    /// The token text.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// `true` when `candidate` equals the token. The time taken does not
    /// depend on where the first difference is.
    pub(crate) fn matches(&self, candidate: &str) -> bool {
        let (a, b) = (self.0.as_bytes(), candidate.as_bytes());
        if a.len() != b.len() {
            return false;
        }
        a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
    }
}

/// The checks every request passes before any route runs.
#[derive(Debug, Clone)]
pub(crate) struct Guard {
    token: Token,
    port: u16,
    /// Host names accepted besides `localhost` and IP literals (the
    /// `--host` name, lowercase).
    names: Vec<String>,
}

impl Guard {
    /// A guard for a server on `port`. `host` is the `--host` value.
    pub(crate) fn new(token: Token, port: u16, host: &str) -> Self {
        let host = host.to_ascii_lowercase();
        let names = if host.parse::<IpAddr>().is_ok() || host == "localhost" {
            Vec::new()
        } else {
            vec![host]
        };
        Self { token, port, names }
    }

    /// The token.
    pub(crate) fn token(&self) -> &Token {
        &self.token
    }

    /// Check `Host`, `Origin`, and `Sec-Fetch-Site`. Returns the `Host`
    /// value.
    ///
    /// # Errors
    ///
    /// 403 `edit.bad_host` for a missing or foreign `Host` (a DNS rebinding
    /// page has its own name there). 403 `edit.bad_origin` for an `Origin`
    /// other than this server, or a cross-site fetch.
    pub(crate) fn check_site<'r>(&self, req: &'r HttpRequest) -> Result<&'r str, HttpError> {
        let host = req.header("host")?.ok_or_else(|| {
            HttpError::new(403, "edit.bad_host", "the request has no Host header")
        })?;
        if !self.host_allowed(host) {
            return Err(HttpError::new(
                403,
                "edit.bad_host",
                format!(
                    "Host '{host}' is not this server; open the URL `zenith edit` printed \
                     (localhost or an IP address, port {})",
                    self.port
                ),
            ));
        }
        if req.header("sec-fetch-site")? == Some("cross-site") {
            return Err(cross_origin("a cross-site fetch"));
        }
        // A browser sends Origin on every POST and on cross-origin GETs.
        // Agents send none, and prove themselves with the token.
        if let Some(origin) = req.header("origin")? {
            let expected = format!("http://{host}");
            if !origin.eq_ignore_ascii_case(&expected) {
                return Err(cross_origin(&format!("Origin '{origin}'")));
            }
        }
        Ok(host)
    }

    /// Check `Authorization: Bearer <token>`. The header is the only
    /// place the token is accepted: no cookie (cookies ignore the port, so
    /// every local service would receive it) and no query (it would land in
    /// logs and history).
    ///
    /// # Errors
    ///
    /// 401 `edit.unauthorized` when no valid token is present.
    pub(crate) fn check_token(&self, req: &HttpRequest) -> Result<(), HttpError> {
        let value = req.header("authorization")?.ok_or_else(unauthorized)?;
        let given = match value.split_once(' ') {
            Some((scheme, rest)) if scheme.eq_ignore_ascii_case("bearer") => rest.trim(),
            Some(_) | None => return Err(unauthorized()),
        };
        if self.token.matches(given) {
            Ok(())
        } else {
            Err(unauthorized())
        }
    }

    fn host_allowed(&self, host: &str) -> bool {
        let (name, port) = split_host(host);
        if port != Some(self.port) && !(port.is_none() && self.port == 80) {
            return false;
        }
        let name = name.to_ascii_lowercase();
        let ip = name
            .strip_prefix('[')
            .and_then(|n| n.strip_suffix(']'))
            .unwrap_or(&name);
        name == "localhost" || ip.parse::<IpAddr>().is_ok() || self.names.contains(&name)
    }
}

/// Split `host[:port]` (with `[v6]` brackets). A port that does not parse
/// reads as `Some(0)`, which matches no server.
pub(crate) fn split_host(host: &str) -> (&str, Option<u16>) {
    let (name, port) = if host.starts_with('[') {
        match host.rfind("]:") {
            Some(i) => (host.get(..=i).unwrap_or(host), host.get(i + 2..)),
            None => (host, None),
        }
    } else {
        match host.rsplit_once(':') {
            Some((n, p)) => (n, Some(p)),
            None => (host, None),
        }
    };
    (name, port.map(|p| p.parse::<u16>().unwrap_or(0)))
}

fn cross_origin(what: &str) -> HttpError {
    HttpError::new(
        403,
        "edit.bad_origin",
        format!("{what} is not this server; send requests from the editor page or without Origin"),
    )
}

fn unauthorized() -> HttpError {
    HttpError::new(
        401,
        "edit.unauthorized",
        "missing or wrong token; send 'Authorization: Bearer <token>' with the token \
         `zenith edit` printed",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::http::Method;

    fn req(method: Method, headers: &[(&str, &str)], query: &str) -> HttpRequest {
        HttpRequest {
            method,
            path: "/".into(),
            query: query.into(),
            headers: headers
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect(),
            length: 0,
            body: Vec::new(),
        }
    }

    fn guard() -> Guard {
        Guard::new(Token("ab".repeat(32)), 4242, "127.0.0.1")
    }

    #[test]
    fn tokens_are_random_hex_and_compare_exactly() {
        let a = Token::generate().expect("token");
        let b = Token::generate().expect("token");
        assert_eq!(a.as_str().len(), 64);
        assert_ne!(a.as_str(), b.as_str());
        assert!(a.matches(a.as_str()));
        assert!(!a.matches(b.as_str()));
        assert!(!a.matches(""));
    }

    #[test]
    fn host_must_name_this_server() {
        let g = guard();
        for ok in [
            "127.0.0.1:4242",
            "localhost:4242",
            "[::1]:4242",
            "LOCALHOST:4242",
        ] {
            assert!(
                g.check_site(&req(Method::Get, &[("host", ok)], "")).is_ok(),
                "{ok}"
            );
        }
        for bad in [
            "evil.example:4242",
            "127.0.0.1:1",
            "127.0.0.1",
            "localhost:x",
        ] {
            let err = g
                .check_site(&req(Method::Get, &[("host", bad)], ""))
                .expect_err(bad);
            assert_eq!(err.code, "edit.bad_host", "{bad}");
        }
        assert!(g.check_site(&req(Method::Get, &[], "")).is_err());
    }

    #[test]
    fn origin_must_be_this_server() {
        let g = guard();
        let host = ("host", "127.0.0.1:4242");
        let ok = req(
            Method::Post,
            &[host, ("origin", "http://127.0.0.1:4242")],
            "",
        );
        assert!(g.check_site(&ok).is_ok());
        assert!(g.check_site(&req(Method::Post, &[host], "")).is_ok());
        for origin in ["http://evil.example", "null", "http://localhost:4242"] {
            let err = g
                .check_site(&req(Method::Post, &[host, ("origin", origin)], ""))
                .expect_err(origin);
            assert_eq!(err.code, "edit.bad_origin");
        }
        let err = g
            .check_site(&req(
                Method::Get,
                &[host, ("sec-fetch-site", "cross-site")],
                "",
            ))
            .expect_err("cross-site");
        assert_eq!(err.code, "edit.bad_origin");
    }

    #[test]
    fn token_comes_only_from_the_bearer_header() {
        let g = guard();
        let t = "ab".repeat(32);
        for scheme in ["Bearer", "bearer", "BEARER"] {
            let value = format!("{scheme} {t}");
            assert_eq!(
                g.check_token(&req(Method::Post, &[("authorization", &value)], "")),
                Ok(()),
                "{scheme}"
            );
        }
        let cookie = format!("zenith_edit_4242={t}");
        let err = g
            .check_token(&req(Method::Get, &[("cookie", &cookie)], ""))
            .expect_err("cookie");
        assert_eq!(err.status, 401);
        let q = format!("token={t}");
        assert!(g.check_token(&req(Method::Get, &[], &q)).is_err(), "query");
        for wrong in ["Bearer nope", "Basic abc", &t, ""] {
            let r = req(Method::Post, &[("authorization", wrong)], "");
            assert_eq!(g.check_token(&r).expect_err(wrong).status, 401, "{wrong}");
        }
    }
}
