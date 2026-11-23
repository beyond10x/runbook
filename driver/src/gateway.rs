//! The gateway file: the address and the token a model program reaches its model through, written by
//! `runbook run` into its private directory and read by the programs of the run (`docs/loom-run-settings.md`,
//! "`[gateway]`"). The one Rust reader of the format; the shell programs of the bindings read it with
//! `bindings/tools/gateway.sh`.
//!
//! The file is two lines: `base_url=<address>` and `token=<token>`. No error and no `Debug` output of
//! this module holds the token, a part of it or the path of a file it was read from.

use std::io::Read;
use std::path::Path;

/// The environment variable that names the gateway file for a program (`sdc-node` sets it from
/// `--gateway-file`; the adapters read it).
pub const FILE_VARIABLE: &str = "SDC_GATEWAY_FILE";
/// The option of `sdc-node` that names the gateway file.
pub const OPTION: &str = "--gateway-file";
/// The most a token has, in bytes.
pub const TOKEN_LIMIT: usize = 8192;
/// The fewest bytes a token has: the adapters replace every occurrence of the token in what a model program
/// prints, so a token short enough to be a common string is refused.
pub const TOKEN_MINIMUM: usize = 8;

/// An address and a token. The token is private to the value: it is read through [`Gateway::token`]
/// and is never in `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Gateway {
    base_url: String,
    token: String,
}

impl std::fmt::Debug for Gateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gateway").field("base_url", &self.base_url).field("token", &"<named>").finish()
    }
}

/// Why `url` is no address of a gateway: `Err` says what is wrong, never the value. An address is
/// `http://` or `https://` (lower case) and a host, then a port and a path if wanted; it has no user
/// information, no query, no fragment, and no blank, control or non-ASCII character.
pub fn check_base_url(url: &str) -> Result<(), &'static str> {
    if url.is_empty() {
        return Err("is empty");
    }
    if !url.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("holds a blank, a control or a non-ASCII character");
    }
    let Some(rest) = url.strip_prefix("http://").or_else(|| url.strip_prefix("https://")) else {
        return Err("is not an http:// or https:// address");
    };
    let (authority, _path) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.contains('@') {
        return Err("holds user information");
    }
    if url.contains('?') || url.contains('#') {
        return Err("holds a query or a fragment");
    }
    let host = if authority.starts_with('[') {
        match authority.split_once(']') {
            Some((inside, after)) if inside.len() > 1 && (after.is_empty() || after.strip_prefix(':').is_some_and(|p| p.bytes().all(|b| b.is_ascii_digit()))) => inside,
            _ => return Err("has no host"),
        }
    } else {
        let host = authority.split(':').next().unwrap_or("");
        if authority.matches(':').count() > 1 || authority.split_once(':').is_some_and(|(_, p)| !p.bytes().all(|b| b.is_ascii_digit())) {
            return Err("has no host");
        }
        host
    };
    if host.is_empty() {
        return Err("has no host");
    }
    Ok(())
}

/// Why `token` is no token: `Err` says what is wrong, never the value. A token is one line of visible
/// ASCII, [`TOKEN_MINIMUM`] to [`TOKEN_LIMIT`] bytes.
pub fn check_token(token: &str) -> Result<(), &'static str> {
    if token.trim().is_empty() {
        return Err("is empty");
    }
    if token.len() > TOKEN_LIMIT {
        return Err("is longer than 8192 bytes");
    }
    if !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("is not one line of visible text");
    }
    if token.len() < TOKEN_MINIMUM {
        return Err("is shorter than 8 bytes");
    }
    Ok(())
}

impl Gateway {
    /// A gateway of `base_url` and `token`, both checked. An error names the member, never its value.
    pub fn new(base_url: &str, token: &str) -> Result<Gateway, String> {
        check_base_url(base_url).map_err(|why| format!("gateway.base_url: {why}"))?;
        check_token(token).map_err(|why| format!("gateway.token: {why}"))?;
        Ok(Gateway { base_url: base_url.to_owned(), token: token.to_owned() })
    }

    /// The address.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The token.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// The text of the gateway file.
    pub fn render(&self) -> String {
        format!("base_url={}\ntoken={}\n", self.base_url, self.token)
    }
}

/// The gateway of the text of a gateway file: exactly the two lines, in that order. An error never
/// holds a part of the text.
pub fn parse(text: &str) -> Result<Gateway, String> {
    let not = || "the gateway file is not a gateway file (two lines: base_url=, token=)".to_owned();
    let mut lines = text.split('\n');
    let base = lines.next().and_then(|l| l.strip_prefix("base_url=")).ok_or_else(not)?;
    let token = lines.next().and_then(|l| l.strip_prefix("token=")).ok_or_else(not)?;
    if !matches!(lines.next(), None | Some("")) || lines.next().is_some() {
        return Err(not());
    }
    Gateway::new(base, token).map_err(|_| not())
}

/// The gateway of the file at `path`. An error says that the file could not be read or is not a gateway
/// file; it holds neither the path nor a part of the content.
pub fn read(path: &Path) -> Result<Gateway, String> {
    let mut text = String::new();
    let file = std::fs::File::open(path).map_err(|_| "the gateway file cannot be read".to_owned())?;
    file.take(2 * (TOKEN_LIMIT as u64) + 1024).read_to_string(&mut text).map_err(|_| "the gateway file cannot be read".to_owned())?;
    parse(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "tok-unit-5f3a91c2";

    #[test]
    fn a_gateway_renders_to_two_lines_and_parses_back() {
        let g = Gateway::new("http://127.0.0.1:8080", TOKEN).unwrap();
        assert_eq!(g.render(), format!("base_url=http://127.0.0.1:8080\ntoken={TOKEN}\n"));
        assert_eq!(parse(&g.render()).unwrap(), g);
        assert_eq!((g.base_url(), g.token()), ("http://127.0.0.1:8080", TOKEN));
    }

    #[test]
    fn debug_never_prints_the_token() {
        let g = Gateway::new("https://gateway.example.test/v1", TOKEN).unwrap();
        let shown = format!("{g:?} {g:#?}");
        assert!(!shown.contains(TOKEN) && !shown.contains("5f3a"), "{shown}");
        assert!(shown.contains("gateway.example.test"));
    }

    #[test]
    fn an_address_is_http_or_https_with_a_host_and_nothing_secret_in_it() {
        for ok in ["http://127.0.0.1:8080", "https://host.example.test", "http://[::1]:9/v1/", "https://h/a/b"] {
            assert_eq!(check_base_url(ok), Ok(()), "{ok}");
        }
        for bad in [
            "", "127.0.0.1:8080", "ftp://h", "HTTP://h", "http://", "http:///path", "https://:80", "http:/h", "http://u:p@h", "http://u@h", "http://h?x=1", "http://h#f", "http://h x",
            "http://h\n", "http://h/\u{7f}",
        ] {
            assert!(check_base_url(bad).is_err(), "{bad:?} was accepted");
        }
    }

    #[test]
    fn a_token_is_one_line_of_visible_text() {
        assert_eq!(check_token(TOKEN), Ok(()));
        assert_eq!(check_token("12345678"), Ok(()), "8 bytes is the fewest");
        for bad in ["", " ", "1234567", "a\nb", "a\rb", "a\u{0}b", "a\tb", "a\u{7f}b", &"x".repeat(TOKEN_LIMIT + 1)] {
            assert!(check_token(bad).is_err(), "{bad:?} was accepted");
        }
    }

    #[test]
    fn a_refusal_holds_no_part_of_the_value() {
        let e = Gateway::new("http://127.0.0.1:1", &format!("{TOKEN}\nmore")).unwrap_err();
        assert!(e.contains("token") && !e.contains(TOKEN) && !e.contains("more"), "{e}");
        let e = Gateway::new("http://user:hunter2@h", TOKEN).unwrap_err();
        assert!(e.contains("base_url") && !e.contains("hunter2"), "{e}");
        for text in ["", "base_url=http://h\n", "token=x\n", &format!("token={TOKEN}\nbase_url=http://h\n"), &format!("base_url=http://h\ntoken={TOKEN}\nextra=1\n"), "base_url=http://h\ntoken=\n"] {
            let e = parse(text).unwrap_err();
            assert!(!e.contains(TOKEN), "{text:?}: {e}");
        }
    }

    #[test]
    fn a_file_is_read_and_a_missing_one_is_refused_without_its_path() {
        let exe = std::env::current_exe().unwrap();
        let dir = exe.parent().unwrap().parent().unwrap().join(format!("gateway-unit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gateway");
        std::fs::write(&path, Gateway::new("http://127.0.0.1:8080", TOKEN).unwrap().render()).unwrap();
        assert_eq!(read(&path).unwrap().token(), TOKEN);
        let e = read(&dir.join("absent-file-name")).unwrap_err();
        assert!(!e.contains("absent-file-name"), "{e}");
        std::fs::write(&path, "garbage\n").unwrap();
        let e = read(&path).unwrap_err();
        assert!(!e.contains("garbage"), "{e}");
    }
}
