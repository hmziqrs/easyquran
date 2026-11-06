use serde::Deserialize;

use crate::db::sea_models::legal_hold::actions::{
    HoldRule, HoldSubject, NewHold, CLASS_CSAM_HOLD_1Y, CLASS_ENFORCEMENT_12M,
    CLASS_LAW_ENFORCEMENT_90D,
};

pub const MAX_SUBJECTS: usize = 50;
pub const MAX_TEXT_CHARS: usize = 200;
pub const MAX_SUBJECT_ID_CHARS: usize = 128;
/// Subject kinds the sweeper and erasure read from `legal_hold_subjects`.
pub const SUBJECT_KINDS: &[&str] = &["user", "media", "comment", "post", "translation"];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldSubjectPayload {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceHoldPayload {
    /// Retention class name: `csam_hold_1y`, `law_enforcement_90d` or `enforcement_12m`.
    pub rule: String,
    pub authority: String,
    pub reason: String,
    pub owner: String,
    #[serde(default)]
    pub case_ref: Option<String>,
    pub subjects: Vec<HoldSubjectPayload>,
}

/// The hold rule for a retention class name. None for anything else.
pub fn parse_rule(raw: &str) -> Option<HoldRule> {
    match raw {
        CLASS_CSAM_HOLD_1Y => Some(HoldRule::CsamHold1y),
        CLASS_LAW_ENFORCEMENT_90D => Some(HoldRule::LawEnforcement90d),
        CLASS_ENFORCEMENT_12M => Some(HoldRule::Enforcement12m),
        _ => None,
    }
}

fn text_ok(raw: &str, max: usize) -> bool {
    let trimmed = raw.trim();
    !trimmed.is_empty() && trimmed.chars().count() <= max
}

fn numeric_kind(kind: &str) -> bool {
    matches!(kind, "media" | "comment" | "post")
}

/// Checks the payload and builds the hold. The error names the first bad field.
pub fn validate_place(payload: PlaceHoldPayload) -> Result<NewHold, &'static str> {
    let rule = parse_rule(payload.rule.trim()).ok_or("rule is not a known hold class")?;
    if !text_ok(&payload.authority, MAX_TEXT_CHARS) {
        return Err("authority is required, at most 200 characters");
    }
    if !text_ok(&payload.reason, MAX_TEXT_CHARS) {
        return Err("reason is required, at most 200 characters");
    }
    if !text_ok(&payload.owner, MAX_TEXT_CHARS) {
        return Err("owner is required, at most 200 characters");
    }
    if let Some(case_ref) = payload.case_ref.as_deref() {
        if !case_ref.trim().is_empty() && !text_ok(case_ref, MAX_TEXT_CHARS) {
            return Err("caseRef is at most 200 characters");
        }
    }
    if payload.subjects.is_empty() || payload.subjects.len() > MAX_SUBJECTS {
        return Err("subjects must hold between 1 and 50 entries");
    }
    let mut subjects = Vec::with_capacity(payload.subjects.len());
    for subject in payload.subjects {
        let kind = subject.kind.trim().to_string();
        if !SUBJECT_KINDS.contains(&kind.as_str()) {
            return Err("subject kind is not supported");
        }
        let id = subject.id.trim().to_string();
        if id.is_empty() || id.chars().count() > MAX_SUBJECT_ID_CHARS {
            return Err("subject id is required, at most 128 characters");
        }
        let user_id = if kind == "user" {
            Some(
                id.parse::<i32>()
                    .map_err(|_| "user subject id must be a number")?,
            )
        } else {
            None
        };
        if numeric_kind(&kind) && !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err("media, comment and post subject ids must be numbers");
        }
        subjects.push(HoldSubject { kind, id, user_id });
    }
    let case_ref = payload
        .case_ref
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty());
    Ok(NewHold {
        rule,
        authority: payload.authority.trim().to_string(),
        reason: payload.reason.trim().to_string(),
        owner: payload.owner.trim().to_string(),
        case_ref,
        subjects,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(rule: &str, kind: &str, id: &str) -> PlaceHoldPayload {
        PlaceHoldPayload {
            rule: rule.to_string(),
            authority: "court order 12".to_string(),
            reason: "preservation".to_string(),
            owner: "ops@example.test".to_string(),
            case_ref: Some("  CASE-7 ".to_string()),
            subjects: vec![HoldSubjectPayload {
                kind: kind.to_string(),
                id: id.to_string(),
            }],
        }
    }

    #[test]
    fn rule_names_map_to_hold_rules_and_nothing_else() {
        assert_eq!(
            parse_rule("enforcement_12m"),
            Some(HoldRule::Enforcement12m)
        );
        assert_eq!(
            parse_rule("law_enforcement_90d"),
            Some(HoldRule::LawEnforcement90d)
        );
        assert_eq!(parse_rule("csam_hold_1y"), Some(HoldRule::CsamHold1y));
        assert_eq!(parse_rule("forever"), None);
    }

    #[test]
    fn valid_user_hold_carries_user_id_and_trimmed_case_ref() {
        let new = validate_place(payload("enforcement_12m", "user", "42")).unwrap();
        assert_eq!(new.subjects[0].user_id, Some(42));
        assert_eq!(new.case_ref.as_deref(), Some("CASE-7"));
    }

    #[test]
    fn media_subject_has_no_user_id_and_must_be_numeric() {
        let new = validate_place(payload("enforcement_12m", "media", "9")).unwrap();
        assert_eq!(new.subjects[0].user_id, None);
        assert!(validate_place(payload("enforcement_12m", "media", "nine")).is_err());
    }

    #[test]
    fn unknown_kind_and_empty_fields_are_refused() {
        assert!(validate_place(payload("enforcement_12m", "device", "1")).is_err());
        let mut bad = payload("enforcement_12m", "user", "1");
        bad.reason = "   ".to_string();
        assert!(validate_place(bad).is_err());
        assert!(validate_place(payload("nope", "user", "1")).is_err());
    }

    #[test]
    fn subject_count_is_bounded() {
        let mut many = payload("enforcement_12m", "user", "1");
        many.subjects = (0..=MAX_SUBJECTS)
            .map(|i| HoldSubjectPayload {
                kind: "user".into(),
                id: i.to_string(),
            })
            .collect();
        assert!(validate_place(many).is_err());
        let mut none = payload("enforcement_12m", "user", "1");
        none.subjects.clear();
        assert!(validate_place(none).is_err());
    }
}
