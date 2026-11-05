//! Exhaustive actual enum tags; field additions require explicit revision review.

pub(super) fn dg1status(value: &lc631_tldg::Dg1Status) -> &'static str {
    match value {
        lc631_tldg::Dg1Status::ParsedWithinBoundedProfiles => "parsed_within_bounded_profiles",
        lc631_tldg::Dg1Status::ParsedWithSyntaxErrors => "parsed_with_syntax_errors",
    }
}

pub(super) fn regionkind(value: &lc631_tldg::Dg1RegionKind) -> &'static str {
    match value {
        lc631_tldg::Dg1RegionKind::Document => "document",
        lc631_tldg::Dg1RegionKind::Paragraph => "paragraph",
        lc631_tldg::Dg1RegionKind::Heading => "heading",
        lc631_tldg::Dg1RegionKind::BlockQuote => "block_quote",
        lc631_tldg::Dg1RegionKind::List => "list",
        lc631_tldg::Dg1RegionKind::ListItem => "list_item",
        lc631_tldg::Dg1RegionKind::Link => "link",
        lc631_tldg::Dg1RegionKind::Emphasis => "emphasis",
        lc631_tldg::Dg1RegionKind::Strong => "strong",
        lc631_tldg::Dg1RegionKind::Table => "table",
        lc631_tldg::Dg1RegionKind::TableRow => "table_row",
        lc631_tldg::Dg1RegionKind::TableCell => "table_cell",
        lc631_tldg::Dg1RegionKind::Prose => "prose",
        lc631_tldg::Dg1RegionKind::InlineCode => "inline_code",
        lc631_tldg::Dg1RegionKind::FencedCode => "fenced_code",
        lc631_tldg::Dg1RegionKind::IndentedCode => "indented_code",
        lc631_tldg::Dg1RegionKind::OpaqueEmbedded => "opaque_embedded",
        lc631_tldg::Dg1RegionKind::OpaqueHtmlBlock => "opaque_html_block",
        lc631_tldg::Dg1RegionKind::OpaqueHtml => "opaque_html",
    }
}

pub(super) fn contentrole(value: &lc631_tldg::Dg1ContentRole) -> &'static str {
    match value {
        lc631_tldg::Dg1ContentRole::OperativeProse => "operative_prose",
        lc631_tldg::Dg1ContentRole::Quotation => "quotation",
        lc631_tldg::Dg1ContentRole::Code => "code",
        lc631_tldg::Dg1ContentRole::Opaque => "opaque",
        lc631_tldg::Dg1ContentRole::Unknown => "unknown",
    }
}

pub(super) fn dg1language(value: &lc631_tldg::Dg1Language) -> &'static str {
    match value {
        lc631_tldg::Dg1Language::Japanese => "japanese",
        lc631_tldg::Dg1Language::English => "english",
        lc631_tldg::Dg1Language::MixedJapaneseEnglish => "mixed_japanese_english",
        lc631_tldg::Dg1Language::Unknown => "unknown",
    }
}

pub(super) fn strength(value: &lc631_tldg::Dg1RequirementStrength) -> &'static str {
    match value {
        lc631_tldg::Dg1RequirementStrength::Must => "must",
        lc631_tldg::Dg1RequirementStrength::Should => "should",
        lc631_tldg::Dg1RequirementStrength::May => "may",
    }
}

pub(super) fn dg1polarity(value: &lc631_tldg::Dg1RequirementPolarity) -> &'static str {
    match value {
        lc631_tldg::Dg1RequirementPolarity::Positive => "positive",
        lc631_tldg::Dg1RequirementPolarity::Negative => "negative",
        lc631_tldg::Dg1RequirementPolarity::Unknown => "unknown",
        lc631_tldg::Dg1RequirementPolarity::Conflict => "conflict",
    }
}

pub(super) fn conditionkind(value: &lc631_tldg::Dg1ConditionKind) -> &'static str {
    match value {
        lc631_tldg::Dg1ConditionKind::Necessary => "necessary",
        lc631_tldg::Dg1ConditionKind::Conditional => "conditional",
        lc631_tldg::Dg1ConditionKind::Temporal => "temporal",
        lc631_tldg::Dg1ConditionKind::Exception => "exception",
    }
}

pub(super) fn residualreason(value: &lc631_tldg::Dg1ResidualReason) -> &'static str {
    match value {
        lc631_tldg::Dg1ResidualReason::NoSupportedInstructionMarker => {
            "no_supported_instruction_marker"
        }
        lc631_tldg::Dg1ResidualReason::QuotedText => "quoted_text",
        lc631_tldg::Dg1ResidualReason::MixedOrUnknownLanguage => "mixed_or_unknown_language",
        lc631_tldg::Dg1ResidualReason::UnresolvedReference => "unresolved_reference",
    }
}

pub(super) fn sourcerole(value: &lc631_tldg::Dg1SourceRole) -> &'static str {
    match value {
        lc631_tldg::Dg1SourceRole::InstructionCandidate => "instruction_candidate",
        lc631_tldg::Dg1SourceRole::Context => "context",
        lc631_tldg::Dg1SourceRole::Quoted => "quoted",
        lc631_tldg::Dg1SourceRole::Code => "code",
        lc631_tldg::Dg1SourceRole::Opaque => "opaque",
        lc631_tldg::Dg1SourceRole::Unsupported => "unsupported",
        lc631_tldg::Dg1SourceRole::Ambiguous => "ambiguous",
        lc631_tldg::Dg1SourceRole::MarkdownSyntax => "markdown_syntax",
    }
}

pub(super) fn bindingstate(value: &crate::DgclBindingState) -> &'static str {
    match value {
        crate::DgclBindingState::ObservedCandidate => "observed_candidate",
        crate::DgclBindingState::NeedsEvidence => "needs_evidence",
        crate::DgclBindingState::PolarityConflict => "polarity_conflict",
    }
}

pub(super) fn dgclparsestate(value: &crate::DgclInstructionParseState) -> &'static str {
    match value {
        crate::DgclInstructionParseState::Parsed => "parsed",
        crate::DgclInstructionParseState::Ambiguous => "ambiguous",
        crate::DgclInstructionParseState::Unresolved => "unresolved",
        crate::DgclInstructionParseState::Unsupported => "unsupported",
        crate::DgclInstructionParseState::BudgetRejected => "budget_rejected",
    }
}
