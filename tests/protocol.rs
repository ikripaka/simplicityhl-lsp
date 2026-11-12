mod support;

use std::fs;

use serde_json::{json, Value};
use simplicity::{human_encoding::Forest, jet::Elements};
use tempfile::TempDir;
use tower_lsp_server::lsp_types::SemanticTokens;

use support::{file_uri, LspProcess};

fn semantic_token_texts(
    response: &Value,
    legend: &Value,
    source: &str,
) -> Vec<(usize, usize, String, String)> {
    assert!(response.get("error").is_none(), "{response}");
    let tokens: SemanticTokens =
        serde_json::from_value(response["result"].clone()).expect("semantic tokens");
    let lines: Vec<Vec<u16>> = source
        .lines()
        .map(|line| line.encode_utf16().collect())
        .collect();
    let mut line = 0;
    let mut column = 0;
    let mut previous_end = 0;
    tokens
        .data
        .iter()
        .map(|token| {
            line += token.delta_line as usize;
            if token.delta_line != 0 {
                column = 0;
                previous_end = 0;
            }
            column += token.delta_start as usize;
            let end = column + token.length as usize;
            assert!(token.length > 0);
            assert!(column >= previous_end, "overlapping tokens");
            previous_end = end;
            let text = lines
                .get(line)
                .and_then(|text| text.get(column..end))
                .expect("token range stays within a source line");
            (
                line,
                column,
                String::from_utf16(text).expect("token boundaries do not split a surrogate pair"),
                legend["tokenTypes"][token.token_type as usize]
                    .as_str()
                    .expect("token type exists in the advertised legend")
                    .to_owned(),
            )
        })
        .collect()
}

#[test]
fn initialize_response_matches_the_public_protocol_fixture() {
    let root = TempDir::new().expect("workspace");
    let root_uri = file_uri(root.path());
    let expected: Value = serde_json::from_str(include_str!("fixtures/initialize_result.json"))
        .expect("initialize fixture");
    let mut server = LspProcess::spawn();

    let response = server.initialize(&root_uri, &json!({}));

    assert_eq!(response.get("result"), Some(&expected));
    assert!(response.get("error").is_none());
    server.shutdown();
}

#[test]
fn raw_simplicity_example_gets_semantic_tokens_over_stdio() {
    let source = include_str!("../examples/verify_equal.simpl");
    let program = Forest::parse::<Elements>(source).expect("example should parse and type-check");
    assert!(program.roots().contains_key("main"));

    let root = TempDir::new().expect("workspace");
    let path = root.path().join("verify_equal.simpl");
    fs::write(&path, source).expect("example on disk");
    let uri = file_uri(&path);
    let mut server = LspProcess::spawn();
    let initialized = server.initialize(&file_uri(root.path()), &json!({}));
    let legend = &initialized["result"]["capabilities"]["semanticTokensProvider"]["legend"];
    server.notify(
        "textDocument/didOpen",
        &json!({
            "textDocument": {
                "uri": uri,
                "languageId": "simplicity",
                "version": 1,
                "text": source
            }
        }),
    );
    let response = server.request(
        2,
        "textDocument/semanticTokens/full",
        &json!({ "textDocument": { "uri": uri } }),
    );
    let original_tokens = semantic_token_texts(&response, legend, source);
    for (text, kind) in [
        ("-- Verify that two 32-bit words are equal.", "comment"),
        ("expected", "variable"),
        ("matches", "variable"),
        ("main", "variable"),
        ("const", "keyword"),
        ("comp", "keyword"),
        ("pair", "keyword"),
        ("0x0000002a", "number"),
        ("1", "type"),
        ("2", "type"),
        ("2^32", "type"),
        (":=", "operator"),
        (":", "operator"),
        ("->", "operator"),
        ("jet_eq_32", "function"),
        ("jet_verify", "function"),
    ] {
        assert!(
            original_tokens
                .iter()
                .any(|token| token.2 == text && token.3 == kind),
            "missing {kind} token for {text}"
        );
    }
    assert!(original_tokens.contains(&(3, 21, "jet_verify".into(), "function".into())));
    server.shutdown();
}

#[test]
fn raw_simplicity_tokens_follow_unsaved_edits_close_and_reopen() {
    let source = include_str!("../examples/verify_equal.simpl");
    let root = TempDir::new().expect("workspace");
    let path = root.path().join("verify_equal.simpl");
    fs::write(&path, source).expect("example on disk");
    let uri = file_uri(&path);
    let mut server = LspProcess::spawn();
    let initialized = server.initialize(&file_uri(root.path()), &json!({}));
    let legend = &initialized["result"]["capabilities"]["semanticTokensProvider"]["legend"];
    server.notify(
        "textDocument/didOpen",
        &json!({
            "textDocument": {
                "uri": uri,
                "languageId": "simplicity",
                "version": 1,
                "text": source
            }
        }),
    );
    let response = server.request(
        2,
        "textDocument/semanticTokens/full",
        &json!({ "textDocument": { "uri": uri } }),
    );
    let original_tokens = semantic_token_texts(&response, legend, source);

    // An unsaved edit shifts every line and leaves an unfinished expression.
    let edited = format!(
        "-- 😀 draft\r\n\r\n{}",
        source
            .replace("jet_verify", "(jet_verify")
            .replace('\n', "\r\n")
    );
    assert!(Forest::parse::<Elements>(&edited).is_err());
    server.notify(
        "textDocument/didChange",
        &json!({
            "textDocument": { "uri": uri, "version": 2 },
            "contentChanges": [{ "text": edited }]
        }),
    );
    let response = server.request(
        3,
        "textDocument/semanticTokens/full",
        &json!({ "textDocument": { "uri": uri } }),
    );
    let edited_tokens = semantic_token_texts(&response, legend, &edited);
    assert!(edited_tokens.contains(&(0, 0, "-- 😀 draft".into(), "comment".into())));
    assert!(edited_tokens.contains(&(5, 22, "jet_verify".into(), "function".into())));
    assert_eq!(
        fs::read_to_string(&path).expect("unchanged disk contents"),
        source
    );

    server.notify(
        "textDocument/didClose",
        &json!({ "textDocument": { "uri": uri } }),
    );
    let response = server.request(
        4,
        "textDocument/semanticTokens/full",
        &json!({ "textDocument": { "uri": uri } }),
    );
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response.get("result"), Some(&Value::Null));

    server.notify(
        "textDocument/didOpen",
        &json!({
            "textDocument": {
                "uri": uri,
                "languageId": "simplicity",
                "version": 1,
                "text": source
            }
        }),
    );
    let response = server.request(
        5,
        "textDocument/semanticTokens/full",
        &json!({ "textDocument": { "uri": uri } }),
    );
    assert_eq!(
        semantic_token_texts(&response, legend, source),
        original_tokens
    );
    server.shutdown();
}

#[test]
fn witness_diagnostics_follow_document_versions_over_stdio() {
    let root = TempDir::new().expect("workspace");
    let root_uri = file_uri(root.path());
    let witness_uri = file_uri(&root.path().join("contract.wit"));
    let mut server = LspProcess::spawn();
    server.initialize(&root_uri, &json!({}));

    server.notify(
        "textDocument/didOpen",
        &json!({
            "textDocument": {
                "uri": witness_uri,
                "languageId": "simplicityhl-witness",
                "version": 1,
                "text": r#"{"amount":{"value":1}}"#
            }
        }),
    );
    let invalid = server.diagnostics(&witness_uri, 1);
    assert!(invalid["params"]["diagnostics"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["message"]
            .as_str()
            .is_some_and(|message| message.contains("missing required 'type'")))));

    server.notify(
        "textDocument/didChange",
        &json!({
            "textDocument": { "uri": witness_uri, "version": 2 },
            "contentChanges": [{ "text": r#"{"amount":{"value":1,"type":"u32"}}"# }]
        }),
    );
    let valid = server.diagnostics(&witness_uri, 2);
    assert_eq!(valid["params"]["diagnostics"], json!([]));
    server.shutdown();
}

#[test]
fn incomplete_import_completion_survives_parse_failure_and_respects_the_feature_gate() {
    let root = TempDir::new().expect("workspace");
    fs::create_dir_all(root.path().join("simf")).expect("root source directory");
    fs::create_dir_all(root.path().join("deps/merkle/simf")).expect("dependency source directory");
    fs::write(
        root.path().join("Simplex.toml"),
        "[dependencies]\nmerkle = { path = 'deps/merkle' }\n",
    )
    .expect("root manifest");
    fs::write(root.path().join("deps/merkle/Simplex.toml"), "").expect("dependency manifest");
    let dependency = root.path().join("deps/merkle/simf/tree.simf");
    fs::write(&dependency, "pub fn root() {}\n").expect("dependency source");
    let dependency = fs::canonicalize(dependency).expect("canonical dependency source");
    let source = "/* 😀 */ use merkle::";
    let root_path = root.path().join("simf/main.simf");
    fs::write(&root_path, source).expect("root source");
    let root_uri = file_uri(root.path());
    let document_uri = file_uri(&root_path);
    let position = source.encode_utf16().count();

    let request_completion = |initialization_options: Value| {
        let mut server = LspProcess::spawn();
        server.initialize(&root_uri, &initialization_options);
        server.notify(
            "textDocument/didOpen",
            &json!({
                "textDocument": {
                    "uri": document_uri,
                    "languageId": "simplicityhl",
                    "version": 1,
                    "text": source
                }
            }),
        );
        let _diagnostics = server.diagnostics(&document_uri, 1);
        let response = server.request(
            2,
            "textDocument/completion",
            &json!({
                "textDocument": { "uri": document_uri },
                "position": { "line": 0, "character": position }
            }),
        );
        server.shutdown();
        response
    };

    let disabled = request_completion(json!({}));
    assert!(disabled["error"].is_null(), "{disabled}");
    assert!(disabled["result"].is_null(), "{disabled}");

    let enabled = request_completion(
        json!({ "simplicityhl": { "experimentalFeatures": { "imports": true } } }),
    );
    assert_eq!(
        enabled["result"],
        json!([{
            "label": "tree",
            "kind": 9,
            "detail": format!("Module file `{}`", dependency.display())
        }])
    );
    assert!(enabled["error"].is_null(), "{enabled}");
}

#[cfg(unix)]
#[test]
fn formatting_replaces_the_unsaved_buffer_with_simfmt_output() {
    let root = TempDir::new().expect("workspace");
    let simfmt = root.path().join("simfmt");
    let written = std::process::Command::new("sh")
        .args([
            "-c",
            r#"printf '#!/bin/sh\nsed "s/(){/() {/"\n' > "$0" && chmod 755 "$0""#,
        ])
        .arg(&simfmt)
        .status()
        .expect("fake simfmt");
    assert!(written.success());
    let document_uri = file_uri(&root.path().join("main.simf"));

    // Multi-byte text on the last line checks the edit ends at the UTF-16 document end.
    let source = "fn main(){}\n// 😀";
    let mut server = LspProcess::spawn();
    server.initialize(
        &file_uri(root.path()),
        &json!({ "simplicityhl": { "formatter": { "path": simfmt } } }),
    );
    server.notify(
        "textDocument/didOpen",
        &json!({
            "textDocument": {
                "uri": document_uri,
                "languageId": "simplicityhl",
                "version": 1,
                "text": source
            }
        }),
    );

    let response = server.request(
        2,
        "textDocument/formatting",
        &json!({
            "textDocument": { "uri": document_uri },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    );

    assert_eq!(
        response["result"],
        json!([{
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 1, "character": 5 }
            },
            "newText": "fn main() {}\n// 😀"
        }]),
        "{response}"
    );
    server.shutdown();
}

fn assert_diagnostics_use_opened_uri(root: &TempDir, document_uri: &str) {
    let root_uri = file_uri(root.path());
    let source = "fn main() -> u32 { false }\n";
    let mut server = LspProcess::spawn();
    server.initialize(&root_uri, &json!({}));
    server.notify(
        "textDocument/didOpen",
        &json!({
            "textDocument": {
                "uri": document_uri,
                "languageId": "simplicityhl",
                "version": 1,
                "text": source
            }
        }),
    );
    let diagnostics = server.diagnostics(document_uri, 1);
    assert_eq!(diagnostics["params"]["uri"], document_uri);
    assert!(
        diagnostics["params"]["diagnostics"]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "{diagnostics}"
    );
    server.shutdown();
}

#[test]
fn diagnostics_preserve_a_dot_segment_editor_uri() {
    let root = TempDir::new().expect("workspace");
    fs::create_dir_all(root.path().join("simf")).expect("source directory");
    fs::write(root.path().join("Simplex.toml"), "").expect("manifest");
    fs::write(
        root.path().join("simf/main.simf"),
        "fn main() -> u32 { false }\n",
    )
    .expect("source");
    let document_uri = format!("{}/simf/../simf/main.simf", file_uri(root.path()));

    assert_diagnostics_use_opened_uri(&root, &document_uri);
}

#[cfg(unix)]
#[test]
fn diagnostics_preserve_a_symlink_editor_uri() {
    let root = TempDir::new().expect("workspace");
    fs::create_dir_all(root.path().join("simf")).expect("source directory");
    fs::write(root.path().join("Simplex.toml"), "").expect("manifest");
    let source_path = root.path().join("simf/main.simf");
    fs::write(&source_path, "fn main() -> u32 { false }\n").expect("source");
    let alias_path = root.path().join("main-link.simf");
    std::os::unix::fs::symlink(source_path, &alias_path).expect("source symlink");
    let document_uri = file_uri(&alias_path);

    assert_diagnostics_use_opened_uri(&root, &document_uri);
}

#[test]
fn definition_resolves_imports_and_calls_across_non_crate_reexports_over_stdio() {
    let root = TempDir::new().expect("workspace");
    fs::create_dir_all(root.path().join("simf")).expect("root source directory");
    fs::create_dir_all(root.path().join("deps/merkle/simf")).expect("dependency source directory");
    fs::create_dir_all(root.path().join("deps/facade/simf")).expect("facade source directory");
    fs::create_dir_all(root.path().join("deps/leaf/simf")).expect("leaf source directory");
    fs::write(
        root.path().join("Simplex.toml"),
        "[dependencies]\nmerkle = { path = 'deps/merkle' }\nfacade = { path = 'deps/facade' }\n",
    )
    .expect("root manifest");
    fs::write(root.path().join("deps/merkle/Simplex.toml"), "").expect("dependency manifest");
    fs::write(
        root.path().join("deps/facade/Simplex.toml"),
        "[dependencies]\nleaf = { path = '../leaf' }\n",
    )
    .expect("facade manifest");
    fs::write(root.path().join("deps/leaf/Simplex.toml"), "").expect("leaf manifest");
    let merkle = root.path().join("deps/merkle/simf/build_root.simf");
    let leaf = root.path().join("deps/leaf/simf/ops.simf");
    fs::write(&merkle, "pub fn get_root() {}\npub fn hash() {}\n").expect("merkle source");
    fs::write(
        root.path().join("deps/facade/simf/smth.simf"),
        "pub use leaf::ops::hash;\n",
    )
    .expect("facade source");
    fs::write(&leaf, "pub fn hash() {}\n").expect("leaf source");
    let source = "use merkle::build_root::{get_root, hash as and_hash};\nuse facade::smth::hash as or_hash;\nfn main() { get_root(); and_hash(); or_hash(); }\n";
    let root_path = root.path().join("simf/main.simf");
    fs::write(&root_path, source).expect("root source");
    let root_uri = file_uri(root.path());
    let document_uri = file_uri(&root_path);
    let merkle_uri = file_uri(&fs::canonicalize(merkle).expect("canonical merkle source"));
    let leaf_uri = file_uri(&fs::canonicalize(leaf).expect("canonical leaf source"));
    let mut server = LspProcess::spawn();
    server.initialize(
        &root_uri,
        &json!({ "simplicityhl": { "experimentalFeatures": { "imports": true } } }),
    );
    server.notify(
        "textDocument/didOpen",
        &json!({
            "textDocument": {
                "uri": document_uri,
                "languageId": "simplicityhl",
                "version": 1,
                "text": source
            }
        }),
    );
    let diagnostics = server.diagnostics(&document_uri, 1);
    assert_eq!(diagnostics["params"]["diagnostics"], json!([]));

    let lines = source.lines().collect::<Vec<_>>();
    let merkle_root = json!({
        "uri": merkle_uri,
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 20 }
        }
    });
    let merkle_hash = json!({
        "uri": merkle_uri,
        "range": {
            "start": { "line": 1, "character": 0 },
            "end": { "line": 1, "character": 16 }
        }
    });
    let leaf_hash = json!({
        "uri": leaf_uri,
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 16 }
        }
    });
    let cases = [
        ("original import", 0, "get_root", &merkle_root),
        ("aliased original import", 0, "hash as", &merkle_hash),
        ("import alias", 0, "and_hash", &merkle_hash),
        ("transitive original import", 1, "hash as", &leaf_hash),
        ("transitive import alias", 1, "or_hash", &leaf_hash),
        ("original call", 2, "get_root", &merkle_root),
        ("aliased call", 2, "and_hash", &merkle_hash),
        ("transitive reexport call", 2, "or_hash", &leaf_hash),
    ];
    for (index, (label, line, needle, expected)) in cases.into_iter().enumerate() {
        let character = lines[line].find(needle).expect("definition token");
        let response = server.request(
            i32::try_from(index).expect("request id") + 2,
            "textDocument/definition",
            &json!({
                "textDocument": { "uri": document_uri },
                "position": { "line": line, "character": character + 1 }
            }),
        );
        assert_eq!(&response["result"], expected, "{label}");
        assert!(response["error"].is_null(), "{label}: {response}");
    }
    server.shutdown();
}
