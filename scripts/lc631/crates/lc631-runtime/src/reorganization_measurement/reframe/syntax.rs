//! Two actual finite languages; declarations are parsed independently of fixtures.
use super::{vec_cap, Error, FrameProfile, ParseCause, ParseRule};
use crate::structural_gain_measurement::{Edge, Vertex};
#[derive(Clone)]
pub(super) struct Decl {
    pub profile: FrameProfile,
    pub entries: Vec<(Edge, Vec<u16>)>,
}
#[derive(Clone, Copy)]
pub(super) struct Term {
    pub edge: Edge,
    pub rule: ParseRule,
}
fn number(s: &str, n: u16) -> Result<u16, Error> {
    if s.is_empty()
        || s.len() > 5
        || !s.bytes().all(|b| b.is_ascii_digit())
        || (s.len() > 1 && s.starts_with('0'))
    {
        return Err(Error::Parse(ParseCause::NonCanonical));
    }
    let v = s
        .parse::<u16>()
        .map_err(|_| Error::Parse(ParseCause::NonCanonical))?;
    if v >= n {
        return Err(Error::Parse(ParseCause::NonCanonical));
    }
    Ok(v)
}
fn edge(s: &str, n: u16) -> Result<Edge, Error> {
    let (a, b) = s
        .split_once(',')
        .ok_or(Error::Parse(ParseCause::InvalidTerm))?;
    Ok(Edge(Vertex(number(a, n)?), Vertex(number(b, n)?)))
}
pub(super) fn declaration(s: &str, n: u16) -> Result<Decl, Error> {
    if s.len() > 16384 || !s.is_ascii() {
        return Err(Error::PayloadLimit);
    }
    let mut fields = s.split(';');
    let profile = match fields.next() {
        Some("frame-v2:direct") => FrameProfile::DirectEdgeV1,
        Some("frame-v2:path") => FrameProfile::CheckedPathMacroV1,
        _ => return Err(Error::Parse(ParseCause::NonCanonical)),
    };
    let name = fields
        .next()
        .and_then(|v| v.strip_prefix("name="))
        .ok_or(Error::Parse(ParseCause::NonCanonical))?;
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(Error::Parse(ParseCause::NonCanonical));
    }
    let mut entries = vec_cap(16)?;
    for item in fields {
        if entries.len() == 16 {
            return Err(Error::PayloadLimit);
        }
        let (key, path) = item
            .split_once('=')
            .ok_or(Error::Parse(ParseCause::NonCanonical))?;
        let e = edge(key, n)?;
        if entries.last().is_some_and(|(last, _)| *last >= e) {
            return Err(Error::Parse(ParseCause::NonCanonical));
        }
        let mut nodes = vec_cap(16)?;
        for node in path.split(',') {
            if nodes.len() == 16 {
                return Err(Error::PayloadLimit);
            }
            nodes.push(number(node, n)?);
        }
        if nodes.len() < 2 {
            return Err(Error::Parse(ParseCause::NonCanonical));
        }
        entries.push((e, nodes));
    }
    Ok(Decl { profile, entries })
}
pub(super) fn parse_direct_v1(s: &str, n: u16) -> Result<Term, Error> {
    if s.starts_with("macro(") {
        return Err(Error::Parse(ParseCause::OutOfFrameLanguage));
    }
    let content = s
        .strip_prefix("edge(")
        .and_then(|x| x.strip_suffix(')'))
        .ok_or(Error::Parse(ParseCause::InvalidTerm))?;
    Ok(Term {
        edge: edge(content, n)?,
        rule: ParseRule::DirectEdgeAtomV1,
    })
}
pub(super) fn parse_checked_path_v1(s: &str, n: u16) -> Result<Term, Error> {
    if let Some(content) = s.strip_prefix("macro(").and_then(|x| x.strip_suffix(')')) {
        return Ok(Term {
            edge: edge(content, n)?,
            rule: ParseRule::CheckedMacroCallV1,
        });
    }
    parse_direct_v1(s, n)
}
