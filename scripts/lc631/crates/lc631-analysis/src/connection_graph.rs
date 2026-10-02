use crate::CodingConnectionPlan;
use lc631_core::stable_sha256;
use lc631_tldg::{
    analyze_dg1, Dg1Budget, Dg1ConditionKind, Dg1Report, Dg1RequirementPolarity, Dg1SyntaxNode,
    Dg1SyntaxTree,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const STATIC_CONNECTION_GRAPH_SCHEMA: &str = "epistesys-static-connection-graph.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StaticConnectionState {
    Connected,
    Disconnected,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionGraphNodeKind {
    SourceRequirement,
    FunctionDefinition,
    ModuleImport,
    CommandRoute,
    CallExpression,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionGraphEdgeKind {
    RequirementTargetCandidate,
    ImportBindsDefinition,
    RouteDispatchesToCall,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConnectionGraphNode {
    pub node_id: String,
    pub file_path: String,
    pub kind: ConnectionGraphNodeKind,
    pub symbol: String,
    pub source_span: std::ops::Range<usize>,
    pub source_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConnectionGraphEdge {
    pub edge_id: String,
    pub kind: ConnectionGraphEdgeKind,
    pub from_node: String,
    pub to_node: String,
    pub state: StaticConnectionState,
    pub source_span: std::ops::Range<usize>,
    pub target_span: std::ops::Range<usize>,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StaticConnectionGraphReport {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub requirement_id: u32,
    pub requirement_source_span: std::ops::Range<usize>,
    pub requirement_source_digest: String,
    pub requirement_target_binding_state: StaticConnectionState,
    pub implementation_path: String,
    pub implementation_digest: String,
    pub router_path: String,
    pub router_digest: String,
    pub implementation_symbol: String,
    pub command: String,
    pub definition_count: usize,
    pub import_bound: bool,
    pub route_arm_count: usize,
    pub matching_call_count: usize,
    pub route_subjects: Vec<String>,
    pub route_input_bound: bool,
    pub configuration_unobserved: bool,
    pub module_symbol_shadowed: bool,
    pub nodes: Vec<ConnectionGraphNode>,
    pub edges: Vec<ConnectionGraphEdge>,
    pub residuals: Vec<String>,
    pub state: StaticConnectionState,
    pub graph_digest: String,
    pub repository_wide_complete: bool,
    pub cross_module_type_resolution: &'static str,
    pub authority_created: bool,
    pub coding_closure_allowed: bool,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum StaticConnectionGraphError {
    InvalidSourceRevision,
    RustParse(String),
    SyntaxErrors,
    MissingRustTree,
    SourceSpanInvalid,
    PayloadEncoding,
}

pub fn analyze_connection_graph_sources(
    plan: &CodingConnectionPlan,
    implementation_source: &str,
    router_source: &str,
) -> Result<StaticConnectionGraphReport, StaticConnectionGraphError> {
    if plan.source_revision != stable_sha256(&plan.source) {
        return Err(StaticConnectionGraphError::InvalidSourceRevision);
    }
    let implementation_report =
        analyze_dg1(implementation_source, Dg1Budget::default()).map_err(|error| {
            StaticConnectionGraphError::RustParse(format!("implementation:{error:?}"))
        })?;
    let router_report = analyze_dg1(router_source, Dg1Budget::default())
        .map_err(|error| StaticConnectionGraphError::RustParse(format!("router:{error:?}")))?;
    let implementation_tree = implementation_report
        .rust_trees
        .first()
        .ok_or(StaticConnectionGraphError::MissingRustTree)?;
    let router_tree = router_report
        .rust_trees
        .first()
        .ok_or(StaticConnectionGraphError::MissingRustTree)?;
    build_connection_graph_from_trees(
        plan,
        &analyze_dg1(&plan.source, Dg1Budget::default())
            .map_err(|error| StaticConnectionGraphError::RustParse(format!("seed:{error:?}")))?,
        implementation_source,
        implementation_tree,
        router_source,
        router_tree,
    )
}

pub(crate) fn build_connection_graph_from_trees(
    plan: &CodingConnectionPlan,
    seed_dg1: &Dg1Report,
    implementation_source: &str,
    implementation_tree: &Dg1SyntaxTree,
    router_source: &str,
    router_tree: &Dg1SyntaxTree,
) -> Result<StaticConnectionGraphReport, StaticConnectionGraphError> {
    if plan.source_revision != stable_sha256(&plan.source) {
        return Err(StaticConnectionGraphError::InvalidSourceRevision);
    }
    if seed_dg1.source_revision != plan.source_revision {
        return Err(StaticConnectionGraphError::InvalidSourceRevision);
    }
    let requirement = seed_dg1
        .requirement_candidates
        .iter()
        .find(|candidate| candidate.id == plan.requirement_id)
        .ok_or(StaticConnectionGraphError::InvalidSourceRevision)?;
    if plan.expected_polarity != polarity_name(requirement.polarity)
        || plan.expected_condition_kind.as_deref() != requirement.condition_kind.map(condition_name)
        || plan.expected_scope != requirement.scope
        || plan.source.get(requirement.source_span.clone())
            != Some(requirement.source_text.as_str())
    {
        return Err(StaticConnectionGraphError::InvalidSourceRevision);
    }
    if implementation_tree.root_has_error || router_tree.root_has_error {
        return Err(StaticConnectionGraphError::SyntaxErrors);
    }
    let implementation_index = RustNodeIndex::new(&implementation_tree.nodes);
    let router_index = RustNodeIndex::new(&router_tree.nodes);
    let module_name = module_name_from_path(&plan.implementation_path);
    let definitions = implementation_tree
        .nodes
        .iter()
        .filter(|node| node.kind == "function_item")
        .filter_map(|node| {
            implementation_index
                .direct_children(node.id)
                .into_iter()
                .find(|child| {
                    child.kind == "identifier"
                        && implementation_source.get(child.span.clone())
                            == Some(plan.implementation_symbol.as_str())
                })
                .map(|name| (node, name))
        })
        .collect::<Vec<_>>();
    let router_root = router_tree
        .nodes
        .iter()
        .find(|node| node.parent_id.is_none())
        .ok_or(StaticConnectionGraphError::MissingRustTree)?;
    let symbol_imports = router_tree
        .nodes
        .iter()
        .filter(|node| node.kind == "use_declaration")
        .filter(|node| {
            router_index.descendants(node.id).into_iter().any(|child| {
                child.kind == "identifier"
                    && router_source.get(child.span.clone())
                        == Some(plan.implementation_symbol.as_str())
            })
        })
        .collect::<Vec<_>>();
    let imports = router_tree
        .nodes
        .iter()
        .filter(|node| node.kind == "use_declaration" && node.parent_id == Some(router_root.id))
        .filter(|node| {
            router_source
                .get(node.span.clone())
                .is_some_and(|text| import_binds(text, &module_name, &plan.implementation_symbol))
        })
        .collect::<Vec<_>>();
    let module_symbol_shadowed = router_index
        .direct_children(router_root.id)
        .into_iter()
        .filter(|node| {
            matches!(
                node.kind.as_str(),
                "function_item" | "const_item" | "static_item"
            )
        })
        .any(|declaration| {
            router_index
                .direct_children(declaration.id)
                .iter()
                .any(|child| {
                    child.kind == "identifier"
                        && router_source.get(child.span.clone())
                            == Some(plan.implementation_symbol.as_str())
                })
        });
    let route_arms = router_tree
        .nodes
        .iter()
        .filter(|node| node.kind == "match_arm")
        .filter(|arm| {
            router_index.descendants(arm.id).into_iter().any(|node| {
                node.kind == "string_literal"
                    && router_source
                        .get(node.span.clone())
                        .is_some_and(|text| string_literal_equals(text, &plan.command))
            })
        })
        .collect::<Vec<_>>();

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut residuals = Vec::new();
    let mut import_bound = imports.len() == 1 && symbol_imports.len() == 1;
    let mut matching_call_count = 0usize;
    let mut configuration_unobserved = false;
    let requirement_text = plan
        .source
        .get(requirement.source_span.clone())
        .ok_or(StaticConnectionGraphError::SourceSpanInvalid)?;
    let requirement_node = make_node(
        "source-instruction",
        ConnectionGraphNodeKind::SourceRequirement,
        &format!("requirement:{}", requirement.id),
        requirement.source_span.clone(),
        requirement_text,
    );
    nodes.push(requirement_node.clone());
    residuals.push(
        "requirement_to_implementation_target_is_plan_declared_not_semantically_verified".into(),
    );

    for (_, name) in &definitions {
        nodes.push(make_node(
            &plan.implementation_path,
            ConnectionGraphNodeKind::FunctionDefinition,
            &plan.implementation_symbol,
            name.span.clone(),
            implementation_source
                .get(name.span.clone())
                .ok_or(StaticConnectionGraphError::SourceSpanInvalid)?,
        ));
    }
    for import in &imports {
        nodes.push(make_node(
            &plan.router_path,
            ConnectionGraphNodeKind::ModuleImport,
            &plan.implementation_symbol,
            import.span.clone(),
            router_source
                .get(import.span.clone())
                .ok_or(StaticConnectionGraphError::SourceSpanInvalid)?,
        ));
    }

    if definitions.len() > 1 {
        residuals.push("duplicate_function_definition".into());
    } else if definitions.is_empty() {
        residuals.push("implementation_definition_missing".into());
    }
    if imports.len() != 1 || symbol_imports.len() != 1 {
        import_bound = false;
        residuals.push(if imports.is_empty() && symbol_imports.is_empty() {
            "module_import_binding_missing".into()
        } else {
            "module_import_binding_ambiguous".into()
        });
    }
    if module_symbol_shadowed {
        residuals.push("same_named_module_definition_shadows_import".into());
    }

    if route_arms.is_empty() {
        residuals.push("declared_command_route_absent_or_dead_string_only".into());
    } else if route_arms.len() > 1 {
        residuals.push("duplicate_command_route_arms".into());
    }
    let route_subjects = route_arms
        .iter()
        .filter_map(|arm| match_subject_for_arm(arm, &router_index, router_source))
        .collect::<Vec<_>>();
    let route_input_bound = route_subjects.len() == route_arms.len()
        && route_subjects.len() == 1
        && matches!(route_subjects[0].as_str(), "command" | "command.as_str()");
    if !route_input_bound {
        residuals.push(if route_subjects.is_empty() {
            "route_match_subject_unresolved".into()
        } else {
            "route_match_subject_does_not_bind_the_cli_command_input".into()
        });
    }

    let mut route_call_edges = Vec::new();
    let mut found_opaque_route = false;
    for arm in &route_arms {
        let route_node = router_index
            .descendants(arm.id)
            .into_iter()
            .find(|node| {
                node.kind == "string_literal"
                    && router_source
                        .get(node.span.clone())
                        .is_some_and(|text| string_literal_equals(text, &plan.command))
            })
            .ok_or(StaticConnectionGraphError::SourceSpanInvalid)?;
        let route_text = router_source
            .get(route_node.span.clone())
            .ok_or(StaticConnectionGraphError::SourceSpanInvalid)?;
        let route_id = make_node(
            &plan.router_path,
            ConnectionGraphNodeKind::CommandRoute,
            &plan.command,
            route_node.span.clone(),
            route_text,
        );
        nodes.push(route_id.clone());

        let arm_descendants = router_index.descendants(arm.id);
        let arm_header = router_source
            .get(arm.span.clone())
            .and_then(|text| text.split_once("=>").map(|(header, _)| header))
            .ok_or(StaticConnectionGraphError::SourceSpanInvalid)?;
        let route_guard_unobserved = arm_header
            .replace(&format!("\"{}\"", plan.command), "")
            .split_whitespace()
            .any(|token| token == "if");
        if route_guard_unobserved {
            configuration_unobserved = true;
            residuals.push("route_match_guard_unobserved".into());
        }
        let calls = arm_descendants
            .iter()
            .filter(|node| node.kind == "call_expression")
            .filter_map(|call| {
                direct_callee_text(call, router_source, &router_index).map(|callee| (call, callee))
            })
            .collect::<Vec<_>>();
        let matching_calls = calls
            .iter()
            .filter(|(_, callee)| {
                callee_resolves_to(callee, &module_name, &plan.implementation_symbol)
            })
            .collect::<Vec<_>>();

        if matching_calls.is_empty()
            && arm_descendants.iter().any(|node| {
                node.kind == "macro_invocation"
                    && router_source
                        .get(node.span.clone())
                        .is_some_and(|text| text.contains(&plan.implementation_symbol))
            })
        {
            found_opaque_route = true;
            residuals.push("opaque_macro_route_requires_compiler_or_rpa_resolution".into());
        } else if matching_calls.is_empty() {
            residuals.push("route_arm_does_not_call_declared_callee".into());
        }

        if matching_calls.len() > 1 {
            residuals.push("route_arm_has_multiple_candidate_calls".into());
        }
        for (call, callee) in matching_calls {
            matching_call_count += 1;
            let call_text = router_source
                .get(call.span.clone())
                .ok_or(StaticConnectionGraphError::SourceSpanInvalid)?;
            let call_node = make_node(
                &plan.router_path,
                ConnectionGraphNodeKind::CallExpression,
                callee,
                call.span.clone(),
                call_text,
            );
            nodes.push(call_node.clone());
            let shadowed = local_binding_shadows(
                call,
                &plan.implementation_symbol,
                router_source,
                &router_index,
            );
            if shadowed {
                residuals.push("local_shadow_binding_prevents_call_resolution".into());
            }
            let function_id = enclosing_function(call, &router_index).map(|node| node.id);
            let cfg_unobserved = function_id
                .is_some_and(|id| function_has_unresolved_cfg(id, router_source, &router_index));
            if cfg_unobserved {
                configuration_unobserved = true;
                residuals.push("feature_or_target_configuration_unobserved".into());
            }
            let resolved = !shadowed
                && !module_symbol_shadowed
                && !cfg_unobserved
                && !route_guard_unobserved
                && definitions.len() == 1
                && imports.len() == 1;
            route_call_edges.push(ConnectionGraphEdge {
                edge_id: stable_sha256(&format!(
                    "connection-edge.v1\0{}\0{}\0{}\0{:?}",
                    route_id.node_id,
                    call_node.node_id,
                    plan.implementation_symbol,
                    if resolved {
                        StaticConnectionState::Connected
                    } else {
                        StaticConnectionState::Unresolved
                    }
                )),
                kind: ConnectionGraphEdgeKind::RouteDispatchesToCall,
                from_node: route_id.node_id.clone(),
                to_node: call_node.node_id.clone(),
                state: if resolved {
                    StaticConnectionState::Connected
                } else {
                    StaticConnectionState::Unresolved
                },
                source_span: route_node.span.clone(),
                target_span: call.span.clone(),
                reason: if resolved {
                    "the declared command match arm contains the direct call and imports the declared symbol".into()
                } else {
                    "route-to-call candidate retained but shadowing, configuration, or import binding remains unresolved".into()
                },
            });
        }
    }

    if definitions.len() == 1 && imports.len() == 1 {
        let (definition, name) = definitions[0];
        let import = imports[0];
        let definition_node = nodes
            .iter()
            .find(|node| {
                node.kind == ConnectionGraphNodeKind::FunctionDefinition
                    && node.source_span == name.span
            })
            .ok_or(StaticConnectionGraphError::SourceSpanInvalid)?;
        let import_node = nodes
            .iter()
            .find(|node| {
                node.kind == ConnectionGraphNodeKind::ModuleImport
                    && node.source_span == import.span
            })
            .ok_or(StaticConnectionGraphError::SourceSpanInvalid)?;
        edges.push(ConnectionGraphEdge {
            edge_id: stable_sha256(&format!(
                "connection-import-edge.v1\0{}\0{}\0{}",
                import_node.node_id, definition_node.node_id, plan.implementation_symbol
            )),
            kind: ConnectionGraphEdgeKind::ImportBindsDefinition,
            from_node: import_node.node_id.clone(),
            to_node: definition_node.node_id.clone(),
            state: StaticConnectionState::Connected,
            source_span: import.span.clone(),
            target_span: name.span.clone(),
            reason: format!(
                "module import `{module_name}` names the unique declared function definition"
            ),
        });
        let _ = definition;
    }
    if definitions.len() == 1 {
        let (_, name) = definitions[0];
        let definition_node = nodes
            .iter()
            .find(|node| {
                node.kind == ConnectionGraphNodeKind::FunctionDefinition
                    && node.source_span == name.span
            })
            .ok_or(StaticConnectionGraphError::SourceSpanInvalid)?;
        edges.push(ConnectionGraphEdge {
            edge_id: stable_sha256(&format!(
                "connection-target-candidate-edge.v1\0{}\0{}\0{}",
                requirement_node.node_id, definition_node.node_id, requirement.id
            )),
            kind: ConnectionGraphEdgeKind::RequirementTargetCandidate,
            from_node: requirement_node.node_id.clone(),
            to_node: definition_node.node_id.clone(),
            state: StaticConnectionState::Unresolved,
            source_span: requirement.source_span.clone(),
            target_span: name.span.clone(),
            reason: "the plan names this target, but no independent semantic mapping proves it implements the user requirement".into(),
        });
    }
    edges.extend(route_call_edges);

    let fully_connected = definitions.len() == 1
        && import_bound
        && route_arms.len() == 1
        && matching_call_count == 1
        && route_input_bound
        && !configuration_unobserved
        && !found_opaque_route
        && edges
            .iter()
            .filter(|edge| edge.kind != ConnectionGraphEdgeKind::RequirementTargetCandidate)
            .all(|edge| edge.state == StaticConnectionState::Connected);
    let state = if fully_connected {
        StaticConnectionState::Connected
    } else if definitions.is_empty()
        || route_arms.is_empty()
        || (matching_call_count == 0 && !found_opaque_route)
        || (!route_subjects.is_empty() && !route_input_bound)
    {
        StaticConnectionState::Disconnected
    } else {
        StaticConnectionState::Unresolved
    };
    residuals.sort();
    residuals.dedup();
    nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    edges.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    let graph_digest = stable_sha256(
        &serde_json::to_string(&(
            STATIC_CONNECTION_GRAPH_SCHEMA,
            &plan.source_revision,
            requirement.id,
            &requirement.source_span,
            stable_sha256(requirement_text),
            &plan.implementation_path,
            &plan.implementation_digest,
            &plan.router_path,
            &plan.router_digest,
            &plan.implementation_symbol,
            &plan.command,
            (
                definitions.len(),
                import_bound,
                route_arms.len(),
                matching_call_count,
                &route_subjects,
                route_input_bound,
                configuration_unobserved,
                module_symbol_shadowed,
            ),
            &nodes,
            &edges,
            &residuals,
            state,
        ))
        .map_err(|_| StaticConnectionGraphError::PayloadEncoding)?,
    );
    Ok(StaticConnectionGraphReport {
        schema_version: STATIC_CONNECTION_GRAPH_SCHEMA,
        source_revision: plan.source_revision.clone(),
        requirement_id: requirement.id,
        requirement_source_span: requirement.source_span.clone(),
        requirement_source_digest: stable_sha256(requirement_text),
        requirement_target_binding_state: StaticConnectionState::Unresolved,
        implementation_path: plan.implementation_path.clone(),
        implementation_digest: plan.implementation_digest.clone(),
        router_path: plan.router_path.clone(),
        router_digest: plan.router_digest.clone(),
        implementation_symbol: plan.implementation_symbol.clone(),
        command: plan.command.clone(),
        definition_count: definitions.len(),
        import_bound,
        route_arm_count: route_arms.len(),
        matching_call_count,
        route_subjects,
        route_input_bound,
        configuration_unobserved,
        module_symbol_shadowed,
        nodes,
        edges,
        residuals,
        state,
        graph_digest,
        repository_wide_complete: false,
        cross_module_type_resolution: "unobserved_requires_explicit_compiler_or_rpa_receipt",
        authority_created: false,
        coding_closure_allowed: false,
        claim_boundary: "source-revision-bound local Rust CST graph for one declared target path; not whole-repository reachability, feature resolution, compiler proof, authority, or closure evidence",
    })
}

fn make_node(
    file_path: &str,
    kind: ConnectionGraphNodeKind,
    symbol: &str,
    span: std::ops::Range<usize>,
    text: &str,
) -> ConnectionGraphNode {
    ConnectionGraphNode {
        node_id: stable_sha256(&format!(
            "connection-node.v1\0{file_path}\0{kind:?}\0{symbol}\0{}\0{}\0{}",
            span.start,
            span.end,
            stable_sha256(text)
        )),
        file_path: file_path.into(),
        kind,
        symbol: symbol.into(),
        source_span: span,
        source_digest: stable_sha256(text),
    }
}

fn module_name_from_path(path: &str) -> String {
    let components = Path::new(path).components().collect::<Vec<_>>();
    let crate_name = components
        .windows(2)
        .find_map(|pair| {
            (pair[0].as_os_str() == "crates")
                .then(|| pair[1].as_os_str().to_string_lossy().replace('-', "_"))
        })
        .unwrap_or_else(|| "unknown_module".into());
    crate_name
}

fn import_binds(declaration: &str, module_name: &str, symbol: &str) -> bool {
    let compact = declaration
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    compact.starts_with(&format!("use{module_name}::"))
        && compact
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .any(|part| part == symbol)
        && !compact.contains(&format!("{symbol}as"))
}

fn match_subject_for_arm(
    arm: &Dg1SyntaxNode,
    index: &RustNodeIndex<'_>,
    source: &str,
) -> Option<String> {
    let match_expression = index
        .ancestors(arm)
        .into_iter()
        .find(|node| node.kind == "match_expression")?;
    let text = source.get(match_expression.span.clone())?.trim_start();
    let subject = text.strip_prefix("match")?.split_once('{')?.0.trim();
    (!subject.is_empty()).then(|| subject.to_string())
}

fn string_literal_equals(literal: &str, expected: &str) -> bool {
    literal
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        == Some(expected)
}

struct RustNodeIndex<'a> {
    by_id: BTreeMap<u32, &'a Dg1SyntaxNode>,
    children: BTreeMap<u32, Vec<&'a Dg1SyntaxNode>>,
}

impl<'a> RustNodeIndex<'a> {
    fn new(nodes: &'a [Dg1SyntaxNode]) -> Self {
        let by_id = nodes.iter().map(|node| (node.id, node)).collect();
        let mut children = BTreeMap::<u32, Vec<&Dg1SyntaxNode>>::new();
        for node in nodes {
            if let Some(parent) = node.parent_id {
                children.entry(parent).or_default().push(node);
            }
        }
        for siblings in children.values_mut() {
            siblings.sort_by_key(|node| (node.span.start, node.span.end));
        }
        Self { by_id, children }
    }

    fn direct_children(&self, parent: u32) -> Vec<&'a Dg1SyntaxNode> {
        self.children.get(&parent).cloned().unwrap_or_default()
    }

    fn descendants(&self, root: u32) -> Vec<&'a Dg1SyntaxNode> {
        let mut output = Vec::new();
        let mut stack = self.direct_children(root);
        while let Some(node) = stack.pop() {
            stack.extend(self.direct_children(node.id));
            output.push(node);
        }
        output.sort_by_key(|node| (node.span.start, node.span.end));
        output
    }

    fn ancestors(&self, node: &Dg1SyntaxNode) -> Vec<&'a Dg1SyntaxNode> {
        let mut output = Vec::new();
        let mut cursor = node.parent_id;
        let mut remaining = self.by_id.len();
        while let Some(parent_id) = cursor {
            let Some(parent) = self.by_id.get(&parent_id).copied() else {
                break;
            };
            output.push(parent);
            if remaining == 0 {
                break;
            }
            remaining -= 1;
            cursor = parent.parent_id;
        }
        output
    }
}

fn direct_callee_text<'a>(
    call: &Dg1SyntaxNode,
    source: &'a str,
    index: &RustNodeIndex<'_>,
) -> Option<&'a str> {
    let arguments = index
        .direct_children(call.id)
        .into_iter()
        .find(|child| child.kind == "arguments")?;
    source
        .get(call.span.start..arguments.span.start)
        .map(str::trim)
}

fn callee_resolves_to(callee: &str, module_name: &str, symbol: &str) -> bool {
    callee == symbol
        || callee.ends_with(&format!("::{symbol}"))
            && (callee.starts_with(&format!("{module_name}::"))
                || callee.starts_with("crate::")
                || callee.starts_with("self::"))
}

fn local_binding_shadows(
    call: &Dg1SyntaxNode,
    symbol: &str,
    source: &str,
    index: &RustNodeIndex<'_>,
) -> bool {
    let call_ancestors = index.ancestors(call);
    let scope_ids = call_ancestors
        .iter()
        .filter(|node| matches!(node.kind.as_str(), "block" | "match_arm"))
        .map(|node| node.id)
        .collect::<BTreeSet<_>>();
    let local_shadow = index.by_id.values().any(|node| {
        node.kind == "let_declaration"
            && node.span.end <= call.span.start
            && scope_ids.contains(&nearest_block_or_arm(node, index).unwrap_or(u32::MAX))
            && source
                .get(node.span.clone())
                .and_then(|text| text.split_once('=').map(|(binding, _)| binding))
                .is_some_and(|binding| {
                    binding
                        .split(|character: char| {
                            !character.is_ascii_alphanumeric() && character != '_'
                        })
                        .any(|part| part == symbol)
                })
    });
    if local_shadow {
        return true;
    }
    call_ancestors.iter().any(|function| {
        function.kind == "function_item"
            && index
                .descendants(function.id)
                .into_iter()
                .filter(|node| node.kind == "parameter")
                .any(|parameter| {
                    source
                        .get(parameter.span.clone())
                        .is_some_and(|text| text.trim_start().starts_with(&format!("{symbol}:")))
                })
    })
}

fn nearest_block_or_arm(node: &Dg1SyntaxNode, index: &RustNodeIndex<'_>) -> Option<u32> {
    index
        .ancestors(node)
        .into_iter()
        .find(|candidate| matches!(candidate.kind.as_str(), "block" | "match_arm"))
        .map(|candidate| candidate.id)
}

fn enclosing_function<'a>(
    node: &Dg1SyntaxNode,
    index: &RustNodeIndex<'a>,
) -> Option<&'a Dg1SyntaxNode> {
    index
        .ancestors(node)
        .into_iter()
        .find(|candidate| candidate.kind == "function_item")
}

fn function_has_unresolved_cfg(function_id: u32, source: &str, index: &RustNodeIndex<'_>) -> bool {
    let Some(function) = index.by_id.get(&function_id).copied() else {
        return false;
    };
    index.by_id.values().any(|attribute| {
        attribute.kind == "attribute_item"
            && attribute.parent_id == function.parent_id
            && attribute.span.end <= function.span.start
            && function.span.start.saturating_sub(attribute.span.end) <= 256
            && source
                .get(attribute.span.clone())
                .is_some_and(|text| text.contains("cfg"))
    })
}

fn polarity_name(value: Dg1RequirementPolarity) -> &'static str {
    match value {
        Dg1RequirementPolarity::Positive => "positive",
        Dg1RequirementPolarity::Negative => "negative",
        Dg1RequirementPolarity::Unknown => "unknown",
        Dg1RequirementPolarity::Conflict => "conflict",
    }
}

fn condition_name(value: Dg1ConditionKind) -> &'static str {
    match value {
        Dg1ConditionKind::Necessary => "necessary",
        Dg1ConditionKind::Conditional => "conditional",
        Dg1ConditionKind::Temporal => "temporal",
        Dg1ConditionKind::Exception => "exception",
    }
}
