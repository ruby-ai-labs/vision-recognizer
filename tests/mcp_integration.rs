//! Black-box MCP integration tests: spawn `vision-recognizer-mcp` as a child process
//! and verify the `initialize` + `tools/list` JSON-RPC handshake.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// AC6: JSON-RPC `initialize` + `tools/list` handshake succeeds and
/// `vision.recognize_image` is present in the tool list.
#[test]
fn tool_list_json_rpc_handshake() {
    // Build the MCP binary first — in CI / nextest the binary is already built.
    let bin_path = env!("CARGO_BIN_EXE_vision-recognizer-mcp");

    let mut child = Command::new(bin_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null()) // tracing goes to stderr, suppress for test
        .spawn()
        .expect("failed to spawn vision-recognizer-mcp");

    let stdin = child.stdin.as_mut().expect("failed to get stdin");
    let stdout = child.stdout.take().expect("failed to get stdout");
    let mut reader = BufReader::new(stdout);

    // Send JSON-RPC initialize request
    let init_request = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.0.1"}}}"#;
    writeln!(stdin, "{init_request}").expect("failed to write initialize");

    // Read initialize response
    let mut init_response = String::new();
    reader
        .read_line(&mut init_response)
        .expect("failed to read initialize response");
    assert!(
        init_response.contains("\"result\""),
        "initialize response must contain 'result', got: {init_response}"
    );

    // Send initialized notification (required by MCP protocol)
    let initialized = r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#;
    writeln!(stdin, "{initialized}").expect("failed to write initialized notification");

    // Send tools/list request
    let tools_request = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;
    writeln!(stdin, "{tools_request}").expect("failed to write tools/list");

    // Read tools/list response
    let mut tools_response = String::new();
    reader
        .read_line(&mut tools_response)
        .expect("failed to read tools/list response");

    assert!(
        tools_response.contains("vision.recognize_image"),
        "tools/list response must contain 'vision.recognize_image', got: {tools_response}"
    );
    assert!(
        tools_response.contains("vision.analyze_video"),
        "tools/list response must contain 'vision.analyze_video', got: {tools_response}"
    );
    assert!(
        tools_response.contains("vision.estimate_portion"),
        "tools/list response must contain 'vision.estimate_portion', got: {tools_response}"
    );

    // Graceful shutdown
    drop(child.stdin.take());
    let _ = child.wait();
}
