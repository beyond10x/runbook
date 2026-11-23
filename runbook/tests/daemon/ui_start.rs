//! The start form of the page the daemon serves (`ui/daemon/start.js`): read as plain text, as `ui_decide.rs` does
//! for the answer to a decision. A Playwright spec (`ui/test/daemon-start.spec.mjs`) looks at the browser.

use std::path::Path;

fn page_file(rel: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui").join(rel)).unwrap_or_else(|e| panic!("ui/{rel}: {e}"))
}

#[test]
fn the_start_form_keeps_the_token_in_memory_sends_no_cookie_and_writes_no_markup() {
    let js = page_file("daemon/start.js");
    for banned in ["localStorage", "sessionStorage", "indexedDB", "document.cookie", "history.", "location.", "innerHTML", "outerHTML", "insertAdjacentHTML", "document.write", "eval(", "new Function", "setAttribute('style'", "XMLHttpRequest", "http://", "https://"] {
        assert!(!js.contains(banned), "ui/daemon/start.js uses {banned}");
    }
    // the command carries the bearer and JSON, and never the cookie
    assert!(js.contains("authorization") && js.contains("application/json") && js.contains("credentials: 'omit'"), "the command's headers");
    assert!(js.contains("/daemon/commands/submit-run") && js.contains("/daemon/views/runbooks"));
    assert!(js.contains("textContent"), "text from the daemon and the person reaches the page as a text node");
    // the inspector's files name none of it: its page has no start form and no way to send the command
    for f in ["web/app.js", "web/index.html", "web/source.js"] {
        let text = page_file(f);
        assert!(!text.contains("submit-run") && !text.contains("/daemon/commands"), "ui/{f} names the command");
    }
}
