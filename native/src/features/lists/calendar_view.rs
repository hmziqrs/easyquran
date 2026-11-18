//! The schedule: a `Calendar` whose value the app owns, weekends off, and
//! every item drawn by `renderItem` (weekday names, month names, chevrons)
//! where the bare component would show numbers.

use chrono::NaiveDate;
use nsh_base::{CalendarEntry, CalendarItemKind, CalendarValue, Date, Matcher, calendar, theme};
use nsh_core::prelude::*;

use super::{heading, note};

/// The month names, by number from 1.
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// The weekday names, `0` Sunday.
const WEEKDAYS: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

/// The day the schedule opens on: a Thursday, so the weekend matcher does
/// not disable it.
fn opening_day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 12).unwrap()
}

/// One item's content: what it is, from `entry`, never its state.
fn calendar_item(entry: CalendarEntry) -> impl IntoElement {
    let label = match entry.kind {
        CalendarItemKind::Previous => "‹".to_owned(),
        CalendarItemKind::Next => "›".to_owned(),
        CalendarItemKind::MonthToggle | CalendarItemKind::Month => {
            let name = MONTHS
                .get(entry.month.max(1) as usize - 1)
                .copied()
                .unwrap_or("?");
            if entry.kind == CalendarItemKind::MonthToggle {
                format!("{name} {}", entry.year)
            } else {
                name[..3].to_owned()
            }
        }
        CalendarItemKind::YearToggle | CalendarItemKind::Year => entry.year.to_string(),
        CalendarItemKind::Weekday => WEEKDAYS[entry.value.clamp(0, 6) as usize].to_owned(),
        CalendarItemKind::Day => entry.value.to_string(),
    };
    let today = entry.today;
    let outside = entry.outside;
    text(label)
        .text_sm()
        .style_when(move || today, |s| s.font_weight(FontWeight::SEMIBOLD))
        .style_when(move || outside, |s| s.opacity(0.4))
}

/// The line under the calendar, of the value the app holds.
fn schedule_note(value: Signal<Date>) -> impl IntoElement {
    let line = move || match value.get() {
        Date::Single(Some(day)) => format!("Selected {day}. Weekends are off."),
        Date::Single(None) => "Pick a day. Weekends are off.".to_owned(),
        Date::Range(start, end) => match (start, end) {
            (Some(start), Some(end)) => format!("Selected {start} to {end}."),
            (Some(start), None) => format!("From {start}: pick the last day."),
            _ => "Pick a range.".to_owned(),
        },
    };
    text(line)
        .test_id("calendar-status")
        .text_sm()
        .text_color(move || theme().muted_foreground)
}

/// The Calendar demo: a controlled date, weekends disabled.
#[component]
pub fn CalendarDemo() -> impl IntoElement {
    let value = signal(Date::from(opening_day()));
    view()
        .flex_1()
        .gap_2()
        .pb_4()
        .child(heading("Calendar").child(note("A date the app owns; press a weekday")))
        .child(
            calendar()
                .value(move || CalendarValue::Controlled(value.get()))
                .disabled_matcher(Matcher::DayOfWeek(vec![0, 6]))
                .header_class("flex-row items-center gap-1 px-1 py-2")
                .item_class(
                    "h-9 rounded-md text-sm items-center justify-center \
                     hover:bg-accent/50 selected:bg-primary selected:text-primary-foreground \
                     disabled:opacity-40",
                )
                .render_item(|entry: &CalendarEntry| calendar_item(entry.clone()))
                .on_change(move |day: &Date| value.set(*day))
                .max_w(px(360.0))
                .p_2()
                .rounded_lg()
                .border_1()
                .border_color(move || theme().border)
                .bg(move || theme().card),
        )
        .child(schedule_note(value))
}
