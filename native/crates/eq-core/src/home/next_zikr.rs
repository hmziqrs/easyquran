//! Zikr-session pickers for the azkar page's recite panel and the home
//! "next" line (PARITY H1, A3; web `lib/home/next-zikr.ts`): which card a
//! card-origin visit binds to, and which card to offer next. Cards are the
//! validated JSON values of `store::home::cards`.

use serde_json::Value;

use super::schedule::{SlotState, slot_state_for, slots_of};
use crate::store::sessions::ReadingThread;

fn str_field<'a>(card: &'a Value, key: &str) -> Option<&'a str> {
    card.get(key).and_then(Value::as_str)
}

/// True once a card with a target has counted its way to that target.
pub fn is_target_done(card: &Value) -> bool {
    let count = card.get("count").and_then(Value::as_i64).unwrap_or(0);
    card.get("target")
        .and_then(Value::as_i64)
        .is_some_and(|t| count >= t)
}

fn is_due_now(card: &Value, now_minute: i64) -> bool {
    matches!(
        slot_state_for(&slots_of(card), now_minute),
        Some(SlotState::Due(_))
    )
}

fn is_snippet_of(card: &Value, slug: &str) -> bool {
    str_field(card, "kind") == Some("snippet") && str_field(card, "slug") == Some(slug)
}

/// `boundCardFor`: the card a card-origin thread names, when it still
/// exists and is a snippet card of this page's slug; else `None` (the
/// caller falls back to the first same-slug card).
pub fn bound_card_for<'a>(
    thread: Option<&ReadingThread>,
    cards: &'a [Value],
    slug: &str,
) -> Option<&'a Value> {
    let thread = thread.filter(|t| t.origin == "card")?;
    let reference = thread.origin_ref.as_deref()?;
    if thread.scope != format!("snippet:{slug}") {
        return None;
    }
    cards
        .iter()
        .find(|c| str_field(c, "id") == Some(reference))
        .filter(|c| is_snippet_of(c, slug))
}

/// `nextZikrCard`: rotate the list to start after `current_id` (the whole
/// list when unknown), drop cards with no reader destination for this flow
/// (zikr phrases, same-slug snippets) and finished cards; a schedule-due
/// card wins, else the first left. `None`: nothing left to offer.
pub fn next_zikr_card<'a>(
    cards: &'a [Value],
    current_id: &str,
    slug: &str,
    now_minute: i64,
) -> Option<&'a Value> {
    let wrapped: Vec<&Value> = match cards
        .iter()
        .position(|c| str_field(c, "id") == Some(current_id))
    {
        Some(i) => cards[i + 1..].iter().chain(&cards[..i]).collect(),
        None => cards.iter().collect(),
    };
    let eligible: Vec<&Value> = wrapped
        .into_iter()
        .filter(|c| str_field(c, "kind") != Some("zikr"))
        .filter(|c| !is_snippet_of(c, slug))
        .filter(|c| !is_target_done(c))
        .collect();
    eligible
        .iter()
        .find(|c| is_due_now(c, now_minute))
        .or_else(|| eligible.first())
        .copied()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn rotates_skips_and_prefers_due() {
        let cards = vec![
            json!({"id": "a", "kind": "snippet", "slug": "kursi", "count": 0}),
            json!({"id": "b", "kind": "zikr", "text": "x", "count": 0}),
            json!({"id": "c", "kind": "surah", "surah": 1, "count": 3, "target": 3}),
            json!({"id": "d", "kind": "surah", "surah": 2, "count": 0}),
            json!({"id": "e", "kind": "surah", "surah": 3, "count": 0,
                   "schedule": [{"id": "s", "label": "Fajr", "time": "05:00"}]}),
        ];
        let id = |v: Option<&Value>| v.and_then(|c| c["id"].as_str()).map(str::to_owned);
        assert_eq!(
            id(next_zikr_card(&cards, "a", "kursi", 12 * 60)),
            Some("d".into())
        );
        assert_eq!(
            id(next_zikr_card(&cards, "a", "kursi", 5 * 60)),
            Some("e".into())
        );
        assert_eq!(
            id(next_zikr_card(&cards, "e", "kursi", 12 * 60)),
            Some("d".into())
        );
        assert_eq!(next_zikr_card(&cards[..3], "a", "kursi", 0), None);
    }
}
