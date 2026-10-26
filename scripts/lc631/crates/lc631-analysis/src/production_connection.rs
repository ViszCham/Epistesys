//! A bounded, registered integration-driver observation, not general call tracing.
use crate::validation_producer::{checked_validation_path, safe_test_name};
use crate::{check_cargo_test_result, CodingEvidenceKind, DgclValidationProducerError as Error};
use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt,
};
use lc631_tldg::{analyze_dg1, Dg1Budget, Dg1SyntaxNode};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;

pub const DIRECT_API_OBSERVER: &str = "cargo-exact-direct-api-driver.v1";
pub const DIRECT_API_COVERAGE: &str = "registered_driver_direct_public_api_and_pinned_assertion";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DgclConnectionSpec {
    pub schema_version: String,
    pub integration_target: String,
    pub test_name: String,
    pub entrypoint: String,
    pub implementation_path: String,
    pub driver_path: String,
    pub driver_digest: String,
    pub observer_id: String,
    pub oracle_id: String,
    pub oracle_revision: String,
    pub coverage: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registration: Option<UntrustedReceipt>,
}

pub fn dgcl_connection_registration_payload(spec: &DgclConnectionSpec) -> String {
    stable_sha256(
        &serde_json::to_string(&(
            "epistesys-dgcl-connection-registration.v1",
            &spec.schema_version,
            &spec.integration_target,
            &spec.test_name,
            &spec.entrypoint,
            &spec.implementation_path,
            &spec.driver_path,
            &spec.driver_digest,
            &spec.observer_id,
            &spec.oracle_id,
            &spec.oracle_revision,
            &spec.coverage,
        ))
        .expect("connection metadata serializes"),
    )
}

pub fn dgcl_connection_registration_scope(spec: &DgclConnectionSpec) -> String {
    format!(
        "dgcl/registered-connection/{}",
        dgcl_connection_registration_payload(spec)
    )
}

pub(crate) fn verify_connection_registration(
    spec: &DgclConnectionSpec,
    verifier: &ReceiptVerifier,
    now: u64,
) -> Result<u64, Error> {
    let receipt = spec
        .registration
        .clone()
        .ok_or(Error::ConnectionNotRegistered)?;
    let expiry = receipt
        .claims()
        .expires_at_epoch()
        .ok_or(Error::ConnectionNotRegistered)?;
    verifier
        .verify(
            receipt,
            &ReceiptPolicy::exact(
                ReceiptClass::VerifierCapability,
                SubjectRevision::checked(&spec.driver_digest)
                    .map_err(|_| Error::ConnectionNotRegistered)?,
                ReceiptScope::checked(dgcl_connection_registration_scope(spec))
                    .map_err(|_| Error::ConnectionNotRegistered)?,
                dgcl_connection_registration_payload(spec),
                now,
            ),
            &mut ReplayGuard::default(),
        )
        .map_err(|_| Error::ConnectionNotRegistered)?;
    Ok(expiry)
}

pub(crate) fn validate_connection_source(
    root: &Path,
    spec: &DgclConnectionSpec,
) -> Result<(), Error> {
    let path = spec.entrypoint.split("::").collect::<Vec<_>>();
    if spec.schema_version != "epistesys-dgcl-connection-spec.v1"
        || !safe_test_name(&spec.integration_target)
        || !identifier(&spec.test_name)
        || path.len() != 2
        || path.iter().any(|part| !identifier(part))
        || spec.driver_path != format!("tests/{}.rs", spec.integration_target)
        || !spec.implementation_path.ends_with(".rs")
        || spec.observer_id != DIRECT_API_OBSERVER
        || spec.coverage != DIRECT_API_COVERAGE
        || spec.oracle_id.is_empty()
        || spec.oracle_id.len() > 256
    {
        return Err(Error::InvalidConnectionDriver);
    }
    let driver = checked_validation_path(root, &spec.driver_path)?;
    let driver = bounded_source(&driver)?;
    if stable_sha256(&driver) != spec.driver_digest {
        return Err(Error::StaleInput);
    }
    validate_driver(&driver, spec)?;
    let implementation =
        bounded_source(&checked_validation_path(root, &spec.implementation_path)?)?;
    let nodes = rust_nodes(&implementation)?;
    let definitions = nodes
        .iter()
        .filter(|node| node.kind == "function_item")
        .filter(|node| {
            children(&nodes, node.id).iter().any(|child| {
                child.kind == "identifier" && text(&implementation, child) == Some(path[1])
            })
        })
        .collect::<Vec<_>>();
    if definitions.len() != 1
        || !children(&nodes, definitions[0].id).iter().any(|child| {
            child.kind == "visibility_modifier" && text(&implementation, child) == Some("pub")
        })
    {
        return Err(Error::InvalidConnectionDriver);
    }
    Ok(())
}

pub(crate) fn metadata_matches(
    stdout: &[u8],
    root: &Path,
    manifest: &Path,
    spec: &DgclConnectionSpec,
) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(stdout) else {
        return false;
    };
    let Some(packages) = value.get("packages").and_then(serde_json::Value::as_array) else {
        return false;
    };
    // `::std::assert_eq!` is only an oracle when Cargo has not supplied an
    // external crate named `std`. Inspect the full resolved graph, not merely
    // the selected package's syntactic driver or a --no-deps metadata summary.
    if !unshadowed_standard_library(&value) {
        return false;
    }
    let Some(package) = packages.iter().find(|package| {
        package
            .get("manifest_path")
            .and_then(serde_json::Value::as_str)
            .and_then(|path| Path::new(path).canonicalize().ok())
            .as_deref()
            == Some(manifest)
    }) else {
        return false;
    };
    let Some(targets) = package.get("targets").and_then(serde_json::Value::as_array) else {
        return false;
    };
    let library = spec.entrypoint.split("::").next().unwrap_or_default();
    let expected_implementation = root.join(&spec.implementation_path).canonicalize().ok();
    let expected_driver = root.join(&spec.driver_path).canonicalize().ok();
    let matches = |kind: &str, name: &str, expected: &Option<std::path::PathBuf>| {
        targets
            .iter()
            .filter(|target| {
                target.get("name").and_then(serde_json::Value::as_str) == Some(name)
                    && target
                        .get("kind")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|kinds| kinds.iter().any(|value| value.as_str() == Some(kind)))
                    && target
                        .get("src_path")
                        .and_then(serde_json::Value::as_str)
                        .and_then(|path| Path::new(path).canonicalize().ok())
                        .as_ref()
                        == expected.as_ref()
            })
            .count()
            == 1
            && expected.is_some()
    };
    matches("lib", library, &expected_implementation)
        && matches("test", &spec.integration_target, &expected_driver)
}

fn unshadowed_standard_library(metadata: &serde_json::Value) -> bool {
    use serde_json::Value;
    let shadow = |name: &Value| {
        name.as_str()
            .is_none_or(|name| name.replace('-', "_") == "std")
    };
    let Some(packages) = metadata.get("packages").and_then(Value::as_array) else {
        return false;
    };
    let Some(nodes) = metadata
        .get("resolve")
        .and_then(|resolve| resolve.get("nodes"))
        .and_then(Value::as_array)
    else {
        return false;
    };
    if packages.is_empty() || nodes.is_empty() {
        return false;
    }
    for package in packages {
        let Some(targets) = package.get("targets").and_then(Value::as_array) else {
            return false;
        };
        let Some(dependencies) = package.get("dependencies").and_then(Value::as_array) else {
            return false;
        };
        // Reject any target named std conservatively, including custom library
        // target names, plus declared dependency names and rename aliases.
        if targets.iter().any(|target| shadow(&target["name"]))
            || dependencies.iter().any(|dependency| {
                shadow(&dependency["name"])
                    || (!dependency["rename"].is_null() && shadow(&dependency["rename"]))
            })
        {
            return false;
        }
    }
    nodes.iter().all(|node| {
        node.get("deps")
            .and_then(Value::as_array)
            .is_some_and(|deps| deps.iter().all(|dependency| !shadow(&dependency["name"])))
    })
}

pub(crate) fn connection_test_observed(stdout: &[u8], spec: &DgclConnectionSpec) -> bool {
    let Ok(output) = std::str::from_utf8(stdout) else {
        return false;
    };
    let expected = format!("test {} ... ok", spec.test_name);
    let passed_names = output
        .lines()
        .filter(|line| line.starts_with("test ") && line.ends_with(" ... ok"))
        .collect::<Vec<_>>();
    passed_names == [expected.as_str()]
        && output
            .lines()
            .any(|line| line.starts_with("test result: ok. 1 passed; 0 failed;"))
        && check_cargo_test_result(CodingEvidenceKind::RuntimeValidation, stdout)
}

fn identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        && value.len() <= 128
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn bounded_source(path: &Path) -> Result<String, Error> {
    let file = std::fs::File::open(path).map_err(|_| Error::InvalidConnectionDriver)?;
    if file
        .metadata()
        .map_err(|_| Error::InvalidConnectionDriver)?
        .len()
        > 16 * 1024
    {
        return Err(Error::InvalidConnectionDriver);
    }
    let mut bytes = Vec::new();
    file.take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::InvalidConnectionDriver)?;
    if bytes.len() > 16 * 1024 {
        return Err(Error::InvalidConnectionDriver);
    }
    String::from_utf8(bytes).map_err(|_| Error::InvalidConnectionDriver)
}

fn rust_nodes(source: &str) -> Result<Vec<Dg1SyntaxNode>, Error> {
    // A code region selects the pinned Rust CST regardless of leading attributes.
    // Choose a fence longer than any source run so source strings cannot end it.
    let longest = source
        .split(|character| character != '~')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "~".repeat(longest.max(2) + 1);
    let prefix = format!("{fence}rust\n");
    let wrapped = format!("{prefix}{source}\n{fence}\n");
    let report = analyze_dg1(
        &wrapped,
        Dg1Budget {
            max_source_bytes: 64 * 1024,
            max_syntax_nodes: 4_096,
            ..Dg1Budget::default()
        },
    )
    .map_err(|_| Error::InvalidConnectionDriver)?;
    if report.rust_trees.len() != 1 || report.rust_trees[0].root_has_error {
        return Err(Error::InvalidConnectionDriver);
    }
    let mut nodes = report.rust_trees.into_iter().next().unwrap().nodes;
    for node in &mut nodes {
        node.span.start = node
            .span
            .start
            .checked_sub(prefix.len())
            .ok_or(Error::InvalidConnectionDriver)?;
        node.span.end = node
            .span
            .end
            .checked_sub(prefix.len())
            .ok_or(Error::InvalidConnectionDriver)?;
        if node.kind == "source_file" {
            node.span.end = node.span.end.min(source.len());
        }
        if source.get(node.span.clone()).is_none() {
            return Err(Error::InvalidConnectionDriver);
        }
    }
    Ok(nodes)
}

fn children(nodes: &[Dg1SyntaxNode], parent: u32) -> Vec<&Dg1SyntaxNode> {
    nodes
        .iter()
        .filter(|node| {
            node.parent_id == Some(parent)
                && node.named
                && !matches!(node.kind.as_str(), "line_comment" | "block_comment")
        })
        .collect()
}

fn text<'a>(source: &'a str, node: &Dg1SyntaxNode) -> Option<&'a str> {
    source.get(node.span.clone())
}

fn validate_driver(source: &str, spec: &DgclConnectionSpec) -> Result<(), Error> {
    let invalid = Error::InvalidConnectionDriver;
    let nodes = rust_nodes(source)?;
    let root = nodes
        .iter()
        .find(|node| node.parent_id.is_none())
        .ok_or(invalid.clone())?;
    let items = children(&nodes, root.id);
    if items.len() != 2
        || items[0].kind != "attribute_item"
        || items[1].kind != "function_item"
        || text(source, items[0])
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            != "#[test]"
    {
        return Err(invalid);
    }
    let function = children(&nodes, items[1].id);
    if function.len() != 3
        || function[0].kind != "identifier"
        || text(source, function[0]) != Some(spec.test_name.as_str())
        || function[1].kind != "parameters"
        || text(source, function[1]) != Some("()")
        || function[2].kind != "block"
    {
        return Err(invalid);
    }
    let statements = children(&nodes, function[2].id);
    if statements.len() != 2 || statements[0].kind != "let_declaration" {
        return Err(invalid);
    }
    let binding = children(&nodes, statements[0].id);
    if binding.len() != 2
        || binding[0].kind != "identifier"
        || text(source, binding[0]) != Some("observed")
        || binding[1].kind != "call_expression"
    {
        return Err(invalid);
    }
    let call = children(&nodes, binding[1].id);
    if call.len() != 2
        || call[0].kind != "scoped_identifier"
        || text(source, call[0]) != Some(spec.entrypoint.as_str())
        || call[1].kind != "arguments"
    {
        return Err(invalid);
    }
    for argument in children(&nodes, call[1].id) {
        if !matches!(
            argument.kind.as_str(),
            "integer_literal"
                | "float_literal"
                | "boolean_literal"
                | "char_literal"
                | "string_literal"
                | "raw_string_literal"
        ) {
            return Err(invalid);
        }
    }
    let assertion = if statements[1].kind == "expression_statement" {
        children(&nodes, statements[1].id)
            .into_iter()
            .next()
            .ok_or(invalid.clone())?
    } else {
        statements[1]
    };
    if assertion.kind != "macro_invocation" {
        return Err(invalid);
    }
    let assertion_parts = children(&nodes, assertion.id);
    let assertion_arguments = assertion_parts
        .last()
        .and_then(|node| text(source, node))
        .unwrap_or_default();
    let first_argument_is_observed = assertion_arguments
        .strip_prefix('(')
        .and_then(|arguments| arguments.trim_start().strip_prefix("observed"))
        .is_some_and(|remaining| remaining.trim_start().starts_with(','));
    if assertion_parts.first().and_then(|node| text(source, node)) != Some("::std::assert_eq")
        || !first_argument_is_observed
        || stable_sha256(
            text(source, statements[1])
                .ok_or(invalid.clone())?
                .trim_end_matches(';'),
        ) != spec.oracle_revision
    {
        return Err(invalid);
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_connection_tests.rs"]
mod tests;
