//! F20 memorization repeat engine (web `lib/audio/repeat.ts`).
//! Source-agnostic: it schedules ayah plays and knows nothing about audio.
//! The web's `runRepeat` is an async loop over injected `play`/`wait`; here
//! it is [`RepeatRun`], a step machine the app's player drives, with the
//! same order: no wait after the last step, nothing more once cancelled.

use serde_json::{Map, Value, json};

use super::{
    policy::{AYAH_REPEAT, DELAY_MS, Limit, MAX_REPEAT_STEPS, SET_REPEAT, clamp_int},
    url::is_surah_number,
};
use crate::kv::KeyValue;

/// `RepeatPlanInput` (numbers as typed; clamped by [`normalize_plan`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RepeatPlanInput {
    pub surah: u16,
    pub from: f64,
    pub to: f64,
    pub ayah_repeat: f64,
    pub set_repeat: f64,
    pub delay_ms: f64,
}

/// `RepeatPlan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatPlan {
    pub surah: u16,
    pub from: u16,
    /// Inclusive; never beyond the surah.
    pub to: u16,
    pub ayah_repeat: u32,
    pub set_repeat: u32,
    pub delay_ms: u64,
}

/// `RepeatStep`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatStep {
    pub surah: u16,
    pub ayah: u16,
    /// Set pass, 1-based.
    pub pass: u32,
    pub passes: u32,
    /// Repeat of this ayah within the pass, 1-based.
    pub ayah_pass: u32,
    pub ayah_passes: u32,
    /// Zero-based position in the whole run.
    pub ordinal: usize,
    pub total: usize,
}

fn clamp_u(value: f64, limit: Limit) -> u64 {
    clamp_int(value, limit).max(0) as u64
}

/// `normalizePlan`: `None` when the surah is invalid, the range empty or
/// inverted, or the run exceeds [`MAX_REPEAT_STEPS`]. `to` is clamped down
/// to `ayah_count`; `from` must already fit.
pub fn normalize_plan(input: RepeatPlanInput, ayah_count: u16) -> Option<RepeatPlan> {
    if !is_surah_number(input.surah) || ayah_count < 1 {
        return None;
    }
    if !input.from.is_finite() || !input.to.is_finite() {
        return None;
    }
    let from = input.from.round();
    let to = input.to.round().min(f64::from(ayah_count));
    if from < 1.0 || from > to {
        return None;
    }
    let plan = RepeatPlan {
        surah: input.surah,
        from: from as u16,
        to: to as u16,
        ayah_repeat: clamp_u(input.ayah_repeat, AYAH_REPEAT) as u32,
        set_repeat: clamp_u(input.set_repeat, SET_REPEAT) as u32,
        delay_ms: clamp_u(input.delay_ms, DELAY_MS),
    };
    (plan_length(&plan) as u64 <= MAX_REPEAT_STEPS).then_some(plan)
}

/// `planLength`.
pub fn plan_length(plan: &RepeatPlan) -> usize {
    plan.set_repeat as usize * usize::from(plan.to - plan.from + 1) * plan.ayah_repeat as usize
}

/// `planSteps`: set passes outer, ayahs in order, per-ayah repeats inner.
pub fn plan_steps(plan: &RepeatPlan) -> Vec<RepeatStep> {
    let total = plan_length(plan);
    let mut steps = Vec::with_capacity(total);
    for pass in 1..=plan.set_repeat {
        for ayah in plan.from..=plan.to {
            for ayah_pass in 1..=plan.ayah_repeat {
                steps.push(RepeatStep {
                    surah: plan.surah,
                    ayah,
                    pass,
                    passes: plan.set_repeat,
                    ayah_pass,
                    ayah_passes: plan.ayah_repeat,
                    ordinal: steps.len(),
                    total,
                });
            }
        }
    }
    steps
}

/// What a [`RepeatRun`] asks for next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatAction {
    /// Play this step (the web's `onStep` then `play`).
    Play(RepeatStep),
    /// Stay silent this long.
    Wait(u64),
    /// The run finished (`"done"`).
    Done,
    /// The run was cancelled (`"cancelled"`).
    Cancelled,
}

/// `runRepeat` as a step machine: call [`RepeatRun::next_action`] after each
/// play ends and after each wait.
#[derive(Debug, Clone)]
pub struct RepeatRun {
    steps: Vec<RepeatStep>,
    delay_ms: u64,
    next: usize,
    /// The last action was a play (a wait may come before the next one).
    played: bool,
    cancelled: bool,
}

impl RepeatRun {
    pub fn new(steps: Vec<RepeatStep>, delay_ms: u64) -> Self {
        Self {
            steps,
            delay_ms,
            next: 0,
            played: false,
            cancelled: false,
        }
    }

    /// Stops the run: every later action is [`RepeatAction::Cancelled`].
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn next_action(&mut self) -> RepeatAction {
        if self.cancelled {
            return RepeatAction::Cancelled;
        }
        if self.next >= self.steps.len() {
            return RepeatAction::Done;
        }
        if self.played && self.delay_ms > 0 {
            self.played = false;
            return RepeatAction::Wait(self.delay_ms);
        }
        let step = self.steps[self.next];
        self.next += 1;
        self.played = true;
        RepeatAction::Play(step)
    }
}

// ---------------------------------------------------------------------------
// Saved presets (device-local).
// ---------------------------------------------------------------------------

/// `RepeatPreset`.
#[derive(Debug, Clone, PartialEq)]
pub struct RepeatPreset {
    pub id: String,
    pub name: String,
    pub input: RepeatPlanInput,
}

/// `REPEAT_PRESETS_KEY`.
pub const REPEAT_PRESETS_KEY: &str = "easyquran.audio.repeatPresets.v1";
/// `MAX_REPEAT_PRESETS`.
pub const MAX_REPEAT_PRESETS: usize = 20;
const MAX_PRESET_NAME: usize = 60;

fn num_in(obj: &Map<String, Value>, key: &str, min: f64, max: f64) -> Option<f64> {
    obj.get(key)?
        .as_f64()
        .filter(|n| n.is_finite() && *n >= min && *n <= max)
}

fn limit_in(obj: &Map<String, Value>, key: &str, limit: Limit) -> Option<f64> {
    num_in(obj, key, limit.min as f64, limit.max as f64)
}

/// `decodePreset`.
fn decode_preset(v: &Value) -> Option<RepeatPreset> {
    let obj = v.as_object()?;
    let id = obj.get("id")?.as_str()?.to_owned();
    let name = obj.get("name")?.as_str()?.to_owned();
    let name_len = name.encode_utf16().count();
    if name_len == 0 || name_len > MAX_PRESET_NAME {
        return None;
    }
    let surah = num_in(obj, "surah", 1.0, 114.0)?;
    if surah.fract() != 0.0 {
        return None;
    }
    Some(RepeatPreset {
        id,
        name,
        input: RepeatPlanInput {
            surah: surah as u16,
            from: num_in(obj, "from", 1.0, 286.0)?,
            to: num_in(obj, "to", 1.0, 286.0)?,
            ayah_repeat: limit_in(obj, "ayahRepeat", AYAH_REPEAT)?,
            set_repeat: limit_in(obj, "setRepeat", SET_REPEAT)?,
            delay_ms: limit_in(obj, "delayMs", DELAY_MS)?,
        },
    })
}

/// `parsePresets`: well-formed presets only, capped.
pub fn parse_presets(raw: &str) -> Vec<RepeatPreset> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(decode_preset)
        .take(MAX_REPEAT_PRESETS)
        .collect()
}

/// A JSON number, integral when the value is (as JS writes it).
fn number(v: f64) -> Value {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        return json!(v as i64);
    }
    json!(v)
}

pub fn encode_presets(presets: &[RepeatPreset]) -> String {
    let items: Vec<Value> = presets
        .iter()
        .map(|p| {
            json!({
                "id": p.id,
                "name": p.name,
                "surah": p.input.surah,
                "from": number(p.input.from),
                "to": number(p.input.to),
                "ayahRepeat": number(p.input.ayah_repeat),
                "setRepeat": number(p.input.set_repeat),
                "delayMs": number(p.input.delay_ms),
            })
        })
        .collect();
    Value::Array(items).to_string()
}

/// `loadRepeatPresets`.
pub fn load_presets(kv: &dyn KeyValue) -> Vec<RepeatPreset> {
    kv.get(REPEAT_PRESETS_KEY)
        .map(|raw| parse_presets(&raw))
        .unwrap_or_default()
}

/// `saveRepeatPresets`: `false` when storage refuses the write.
pub fn save_presets(kv: &dyn KeyValue, presets: &[RepeatPreset]) -> bool {
    kv.set(REPEAT_PRESETS_KEY, &encode_presets(presets)).is_ok()
}

/// `addPreset`: at the front; past the cap the oldest drops off.
pub fn add_preset(presets: &[RepeatPreset], preset: RepeatPreset) -> Vec<RepeatPreset> {
    let mut out = vec![preset.clone()];
    out.extend(presets.iter().filter(|p| p.id != preset.id).cloned());
    out.truncate(MAX_REPEAT_PRESETS);
    out
}

/// `removePreset`.
pub fn remove_preset(presets: &[RepeatPreset], id: &str) -> Vec<RepeatPreset> {
    presets.iter().filter(|p| p.id != id).cloned().collect()
}

/// A fresh preset id (the web's `newEntityId`, UUID v7).
pub fn new_preset_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(from: f64, to: f64, a: f64, s: f64, d: f64) -> RepeatPlanInput {
        RepeatPlanInput {
            surah: 1,
            from,
            to,
            ayah_repeat: a,
            set_repeat: s,
            delay_ms: d,
        }
    }

    #[test]
    fn plan_clamps_and_rejects() {
        let p = normalize_plan(input(1.0, 99.0, 50.0, 0.0, -5.0), 7).unwrap();
        assert_eq!(
            (p.from, p.to, p.ayah_repeat, p.set_repeat, p.delay_ms),
            (1, 7, 20, 1, 0)
        );
        assert!(normalize_plan(input(4.0, 3.0, 1.0, 1.0, 0.0), 7).is_none());
        assert!(normalize_plan(input(8.0, 9.0, 1.0, 1.0, 0.0), 7).is_none());
        assert!(normalize_plan(input(f64::NAN, 3.0, 1.0, 1.0, 0.0), 7).is_none());
        // 286 ayahs * 20 * 100 is far past MAX_REPEAT_STEPS.
        let mut big = input(1.0, 286.0, 20.0, 100.0, 0.0);
        big.surah = 2;
        assert!(normalize_plan(big, 286).is_none());
    }

    #[test]
    fn steps_order_set_outer_repeat_inner() {
        let p = normalize_plan(input(1.0, 2.0, 2.0, 2.0, 0.0), 7).unwrap();
        let steps = plan_steps(&p);
        let order: Vec<(u32, u16, u32)> = steps
            .iter()
            .map(|s| (s.pass, s.ayah, s.ayah_pass))
            .collect();
        assert_eq!(
            order,
            [
                (1, 1, 1),
                (1, 1, 2),
                (1, 2, 1),
                (1, 2, 2),
                (2, 1, 1),
                (2, 1, 2),
                (2, 2, 1),
                (2, 2, 2)
            ]
        );
        assert!(steps.iter().all(|s| s.total == 8));
    }

    #[test]
    fn run_waits_between_but_not_after() {
        let p = normalize_plan(input(1.0, 2.0, 1.0, 1.0, 300.0), 7).unwrap();
        let mut run = RepeatRun::new(plan_steps(&p), p.delay_ms);
        assert!(matches!(run.next_action(), RepeatAction::Play(s) if s.ayah == 1));
        assert_eq!(run.next_action(), RepeatAction::Wait(300));
        assert!(matches!(run.next_action(), RepeatAction::Play(s) if s.ayah == 2));
        assert_eq!(run.next_action(), RepeatAction::Done);
        let mut run = RepeatRun::new(plan_steps(&p), p.delay_ms);
        run.next_action();
        run.cancel();
        assert_eq!(run.next_action(), RepeatAction::Cancelled);
    }

    #[test]
    fn presets_round_trip_and_cap() {
        let preset = |id: &str| RepeatPreset {
            id: id.into(),
            name: "n".into(),
            input: input(1.0, 3.0, 2.0, 1.0, 500.0),
        };
        let mut list = Vec::new();
        for i in 0..25 {
            list = add_preset(&list, preset(&i.to_string()));
        }
        assert_eq!(list.len(), MAX_REPEAT_PRESETS);
        assert_eq!(list[0].id, "24");
        let raw = encode_presets(&list);
        assert!(raw.contains("\"delayMs\":500"));
        assert_eq!(parse_presets(&raw), list);
        assert_eq!(remove_preset(&list, "24").len(), MAX_REPEAT_PRESETS - 1);
        assert!(parse_presets("{}").is_empty());
        assert!(parse_presets(r#"[{"id":"a","name":"","surah":1,"from":1,"to":1,"ayahRepeat":1,"setRepeat":1,"delayMs":0}]"#).is_empty());
    }
}
