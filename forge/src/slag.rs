//! `forge/src/slag.rs` — the §8 `#[slag]` escape hatch semantics: the only
//! sanctioned way to ship a function whose body is NOT machine-proved. The parser
//! already builds the attribute node (`thermite_syntax::FnItem.slag:
//! Option<SlagAttr { reason, owner, review, span }>`); this component supplies the
//! FORGE-side semantics the parser deferred "downstream/forge":
//! (1) **validate** the three mandatory fields are present (`Some`) AND non-empty
//!     after `trim`;
//! (2) **L3-exempt / L1-enforced** — a valid `#[slag]` item is NOT sent to verus;
//!     it certifies at **L1** with `slag: true` + its metadata
//!     (`manifest::Certificate::slag_l1`);
//! (3) it is the ONLY justification for a maximal `fx` row (the §7.1 (d)
//!     interaction — `vacuity.rs` reads `FnItem.slag.is_some()`);
//! (4) it is VISIBLE in the audit surface (the cert carries `slag: true` +
//!     reason/owner/review).
//!
//! Slag exempts you from *proving*, NEVER from *stating and checking*: a valid
//! `#[slag]` item is STILL subject to triage (a)/(b)/(c) (`vacuity.rs`); only the
//! maximal-`fx` (d) check is justified by slag.
//!
//! Governing design: `.design/forge/slag.md`.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (mandatory-field validation) | SHIPPED | `pub fn validate(&SlagAttr) -> Result<SlagMeta, SlagError>` checks each of `reason`/`owner`/`review` is `Some` and non-empty after `trim`; `None` → `SlagError::MissingField`, empty/whitespace → `SlagError::EmptyField`. Consumer: `check::check_file`. |
//! | REQ-2 (L3-exempt / L1-enforced / `slag: true`) | SHIPPED | a valid slag item is NOT lowered/verified by `check::check_file`; `Certificate::slag_l1` (manifest.rs) emits `Level::L1`, `slag: true`, the metadata, and a fiat-trusted discharged obligation (no verus run). |
//! | REQ-3 (slag justifies maximal `fx`) | SHIPPED | `vacuity::triage` reads `item.slag` and skips rule (d) when present; this module's `validate` gates whether that skip is honored (a slag item with INVALID fields is rejected before triage). |
//! | REQ-4 (audit visibility — cert metadata) | SHIPPED | `validate` returns `SlagMeta { reason, owner, review }`; `Certificate.slag_meta` (manifest.rs additive field) carries it into the cert; `cli::render_human` prints it. |
//! | REQ-5 (typed verdict + check integration) | SHIPPED | `SlagError` + `Result<SlagMeta, SlagError>`; `check::check_file` runs validate → triage(a/b/c) → `Certificate::slag_l1` short-circuit (no verus); a `slag.is_none()` item is untouched (normal L3 path). |

use std::fmt;

use thermite_syntax::SlagAttr;

pub use crate::manifest::SlagMeta;

/// The structured cause a `#[slag]` attribute fails validation (REQ-1, REQ-5).
/// Names the offending field so the certificate's reject diagnostic is precise
/// (§8: the fields "are mandatory and non-empty (checked)"). The `tag` is the
/// stable cause string the conformance oracle (`conformance/slag/slag.json`) keys
/// on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlagError {
    /// A mandatory field was omitted entirely (parsed to `None`).
    MissingField { field: &'static str },
    /// A mandatory field was present but empty / whitespace-only after `trim`.
    EmptyField { field: &'static str },
}

impl SlagError {
    /// The stable cause tag the conformance oracle keys on. Matches the
    /// `"cause"` strings in `slag.json` (`SlagFieldMissing` / `SlagFieldEmpty`).
    pub fn tag(&self) -> &'static str {
        match self {
            SlagError::MissingField { .. } => "SlagFieldMissing",
            SlagError::EmptyField { .. } => "SlagFieldEmpty",
        }
    }

    /// The offending field name.
    pub fn field(&self) -> &'static str {
        match self {
            SlagError::MissingField { field } | SlagError::EmptyField { field } => field,
        }
    }

    /// A human-readable diagnostic naming the offending field (§8).
    pub fn detail(&self) -> String {
        let field = self.field();
        match self {
            SlagError::MissingField { .. } => {
                format!("§8: `#[slag]` field `{field}` is mandatory but was omitted")
            }
            SlagError::EmptyField { .. } => {
                format!("§8: `#[slag]` field `{field}` is mandatory and must be non-empty")
            }
        }
    }
}

impl fmt::Display for SlagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.detail())
    }
}

impl std::error::Error for SlagError {}

/// Validate a `#[slag]` attribute's three mandatory fields (REQ-1): each of
/// `reason`/`owner`/`review` must be present (`Some`) AND non-empty after `trim`.
/// Returns the validated [`SlagMeta`] of trimmed owned strings on success, or the
/// first offending field as a [`SlagError`]. Order is `reason`, `owner`, `review`
/// (deterministic, R-CODE-5) — the first failure is reported.
///
/// No panic, no `unwrap` (R-CODE-2): the result is a typed `Result` consumed by
/// `check::check_file`.
pub fn validate(slag: &SlagAttr) -> Result<SlagMeta, SlagError> {
    let reason = validate_field("reason", slag.reason.as_deref())?;
    let owner = validate_field("owner", slag.owner.as_deref())?;
    let review = validate_field("review", slag.review.as_deref())?;
    Ok(SlagMeta {
        reason,
        owner,
        review,
    })
}

/// Validate one mandatory field: `None` → `MissingField`; `Some` but empty after
/// `trim` → `EmptyField`; otherwise the trimmed owned string. The value stored is
/// the TRIMMED form (a `"  required  "` field is recorded as `"required"`).
fn validate_field(field: &'static str, value: Option<&str>) -> Result<String, SlagError> {
    match value {
        None => Err(SlagError::MissingField { field }),
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Err(SlagError::EmptyField { field })
            } else {
                Ok(trimmed.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thermite_syntax::Span;

    fn attr(reason: Option<&str>, owner: Option<&str>, review: Option<&str>) -> SlagAttr {
        SlagAttr {
            reason: reason.map(|s| s.to_string()),
            owner: owner.map(|s| s.to_string()),
            review: review.map(|s| s.to_string()),
            span: Span::new(0, 0),
        }
    }

    // REQ-1 / AC-5: all three present + non-empty → Ok, trimmed.
    #[test]
    fn all_present_non_empty_ok() {
        let a = attr(Some("vendored"), Some("agent:forge-7"), Some("required"));
        let result = validate(&a);
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        if let Ok(meta) = result {
            assert_eq!(meta.reason, "vendored");
            assert_eq!(meta.owner, "agent:forge-7");
            assert_eq!(meta.review, "required");
        }
    }

    // REQ-1 / AC-2: empty `reason` → EmptyField{reason} (cause SlagFieldEmpty).
    #[test]
    fn empty_reason_rejected() {
        let a = attr(Some(""), Some("o"), Some("r"));
        let result = validate(&a);
        assert!(result.is_err(), "empty reason must reject: {result:?}");
        if let Err(e) = result {
            assert_eq!(e.tag(), "SlagFieldEmpty");
            assert_eq!(e.field(), "reason");
        }
    }

    // REQ-1 / AC-2: omitted `owner` → MissingField{owner} (cause SlagFieldMissing).
    #[test]
    fn missing_owner_rejected() {
        let a = attr(Some("x"), None, Some("r"));
        let result = validate(&a);
        assert!(result.is_err(), "missing owner must reject: {result:?}");
        if let Err(e) = result {
            assert_eq!(e.tag(), "SlagFieldMissing");
            assert_eq!(e.field(), "owner");
        }
    }

    // REQ-1 / AC-5: whitespace-only field is EMPTY after trim → EmptyField (whole
    // class — present-but-blank for every field).
    #[test]
    fn whitespace_only_is_empty() {
        for (r, o, v, want_field) in [
            (Some("   "), Some("o"), Some("r"), "reason"),
            (Some("x"), Some("\t"), Some("r"), "owner"),
            (Some("x"), Some("o"), Some(" \n "), "review"),
        ] {
            let a = attr(r, o, v);
            let result = validate(&a);
            assert!(
                result.is_err(),
                "whitespace `{want_field}` must reject: {result:?}"
            );
            if let Err(e) = result {
                assert_eq!(e.tag(), "SlagFieldEmpty");
                assert_eq!(e.field(), want_field);
            }
        }
    }

    // REQ-1: the stored value is TRIMMED.
    #[test]
    fn validated_fields_are_trimmed() {
        let a = attr(Some("  reason  "), Some(" owner "), Some(" required "));
        let result = validate(&a);
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        if let Ok(meta) = result {
            assert_eq!(meta.reason, "reason");
            assert_eq!(meta.owner, "owner");
            assert_eq!(meta.review, "required");
        }
    }

    // REQ-1: `reason` is checked FIRST (deterministic order) — a missing reason
    // is reported even when owner is also bad.
    #[test]
    fn reason_checked_first() {
        let a = attr(None, None, None);
        let result = validate(&a);
        assert!(result.is_err(), "all-missing must reject: {result:?}");
        if let Err(e) = result {
            assert_eq!(e.field(), "reason");
        }
    }
}
