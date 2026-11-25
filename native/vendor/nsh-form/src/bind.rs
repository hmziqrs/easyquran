use std::cell::Cell;
use std::rc::Rc;

use nsh_base::ColorPickerEvent;
use nsh_base::rich_editor::view::{RichEditor, RichEditorApi, RichEditorChange, RichEditorElement};
use nsh_base::{
    CalendarElement, CalendarValue, CheckboxElement, ColorPickerElement, ComboboxElement, Date,
    DatePickerElement, DatePickerValue, DateTime, EditorElement, Input, NumberInputElement,
    OtpInputElement, PickedDate, RadioElement, SelectElement, SliderElement, SliderValue,
    SwitchElement, TimeFieldElement, TimeOfDay, ToggleElement,
};
use nsh_core::JsonValue;
use nsh_core::prelude::*;

use crate::field::Field;

/// Connects a field to a text input: its value, edits, blur and the focus a
/// failed submit moves to it.
pub trait BindField {
    fn bind(self, input: Input) -> Input;
}

impl<H: Clone + PartialEq + 'static> BindField for Field<H, String> {
    fn bind(self, input: Input) -> Input {
        let reference = element_ref();
        self.bind_input(reference);
        input
            .element_ref(&reference)
            .value(move || self.value())
            .on_change(move |text: &str| self.handle_change(text.to_owned()))
            .on_blur(move || self.handle_blur())
    }
}

impl<H: Clone + PartialEq + 'static> Field<H, bool> {
    /// Binds a switch: it shows the value, and a press sets the value.
    pub fn bind_switch(self, switch: SwitchElement) -> SwitchElement {
        switch
            .checked(move || self.value())
            .on_change(move |next: &bool| self.handle_change(*next))
    }

    /// Binds a checkbox: it shows the value, and a press sets the value.
    pub fn bind_checkbox(self, checkbox: CheckboxElement) -> CheckboxElement {
        checkbox
            .checked(move || self.value())
            .on_change(move |next: &bool| self.handle_change(*next))
    }
}

impl<H: Clone + PartialEq + 'static> Field<H, Option<SharedString>> {
    /// Binds a select: it shows the chosen value, and a choice sets it. `None` is no choice.
    pub fn bind_select(self, select: SelectElement) -> SelectElement {
        select
            .value(move || self.value())
            .on_change(move |next: &Option<SharedString>| self.handle_change(next.clone()))
    }
}

impl<H: Clone + PartialEq + 'static> Field<H, Option<f64>> {
    /// Binds a number input: it shows the number, and an edit sets it. `None` is an empty field.
    pub fn bind_number_input(self, input: NumberInputElement) -> NumberInputElement {
        input
            .value(move || self.value())
            .on_change(move |next: &Option<f64>| self.handle_change(*next))
    }
}

impl<H: Clone + PartialEq + 'static> Field<H, bool> {
    /// Binds a toggle: it shows the pressed state, and a press sets the value.
    pub fn bind_toggle(self, toggle: ToggleElement) -> ToggleElement {
        toggle
            .pressed(move || self.value())
            .on_change(move |next: &bool| self.handle_change(*next))
    }
}

impl<H: Clone + PartialEq + 'static> Field<H, Option<Vec<SharedString>>> {
    /// Binds a combobox to the chosen values. `None` means no choice yet.
    pub fn bind_combobox(self, combobox: ComboboxElement) -> ComboboxElement {
        combobox
            .value(move || self.value())
            .on_change(move |next: &Vec<SharedString>| self.handle_change(Some(next.clone())))
    }
}

impl<H: Clone + PartialEq + 'static> Field<H, Option<SharedString>> {
    /// Binds a code editor to its text. `None` is an empty document.
    pub fn bind_editor(self, editor: EditorElement) -> EditorElement {
        editor
            .value(move || self.value())
            .on_change(move |next: &SharedString| self.handle_change(Some(next.clone())))
    }
}

impl<H: Clone + PartialEq + 'static, T: Clone + PartialEq + 'static> Field<H, Vec<T>> {
    /// Binds one toggle of a group to the list: the toggle is pressed while the list holds `item`,
    /// and a press adds or removes `item`. Cf. TanStack's `toggleButtonProps(field, item)`.
    pub fn bind_toggle_item(self, item: T, toggle: ToggleElement) -> ToggleElement {
        let shown = item.clone();
        toggle
            .pressed(move || self.value().contains(&shown))
            .on_change(move |next: &bool| {
                let mut list = self.value_untracked();
                list.retain(|value| value != &item);
                if *next {
                    list.push(item.clone());
                }
                self.handle_change(list);
            })
    }
}

impl<H: Clone + PartialEq + 'static, T: Clone + PartialEq + 'static> Field<H, T> {
    /// Binds one radio of a group: it is checked while the field holds `option`, and a press sets the
    /// field to `option`. Cf. TanStack's `radioProps(field, option)`.
    pub fn bind_radio(self, option: T, radio: RadioElement) -> RadioElement {
        let shown = option.clone();
        radio
            .checked(move || self.value() == shown)
            .on_change(move |next: &bool| {
                if *next {
                    self.handle_change(option.clone());
                }
            })
    }
}

impl<H: Clone + PartialEq + 'static> Field<H, Option<SharedString>> {
    /// Binds a one-time-code input: it shows the typed code, and each edit sets it.
    pub fn bind_otp_input(self, otp: OtpInputElement) -> OtpInputElement {
        otp.value(move || self.value())
            .on_change(move |next: &SharedString| self.handle_change(Some(next.clone())))
    }
}

impl<H: Clone + PartialEq + 'static> Field<H, Option<SliderValue>> {
    /// Binds a slider: a single value or a range, set by a drag or a key.
    pub fn bind_slider(self, slider: SliderElement) -> SliderElement {
        slider
            .value(move || self.value())
            .on_change(move |next: &SliderValue| self.handle_change(Some(*next)))
    }
}

impl<H: Clone + PartialEq + 'static> Field<H, Date> {
    /// Binds a calendar: it shows the date, and a picked day sets it.
    pub fn bind_calendar(self, calendar: CalendarElement) -> CalendarElement {
        calendar
            .value(move || CalendarValue::from(self.value()))
            .on_change(move |next: &Date| self.handle_change(*next))
    }
}

impl<H: Clone + PartialEq + 'static> Field<H, DateTime> {
    /// Binds a date picker: it shows the date or range, and a pick sets it.
    pub fn bind_date_picker(self, picker: DatePickerElement) -> DatePickerElement {
        picker
            .value(move || DatePickerValue::from(self.value()))
            .on_change(move |picked: &PickedDate| self.handle_change(picked.value))
    }
}

impl<H: Clone + PartialEq + 'static> Field<H, Option<TimeOfDay>> {
    /// Binds a time field: typed segments set the time.
    pub fn bind_time_field(self, time: TimeFieldElement) -> TimeFieldElement {
        time.value(move || self.value())
            .on_change(move |next: &TimeOfDay| self.handle_change(Some(*next)))
    }
}

impl<H: Clone + PartialEq + 'static> Field<H, Option<Hsla>> {
    /// Binds a colour picker: it shows the colour, and a change sets it.
    pub fn bind_color_picker(self, picker: ColorPickerElement) -> ColorPickerElement {
        picker
            .value(move || self.value())
            .on_change(move |event: &ColorPickerEvent| {
                if let ColorPickerEvent::Change(color, _formatted) = event {
                    self.handle_change(Some(*color));
                }
            })
    }
}

impl<H: Clone + PartialEq + 'static> Field<H, Option<JsonValue>> {
    /// Binds a rich editor to its document as ProseMirror JSON. The change event carries a revision
    /// only, so the document is read through the editor's ref. A read whose revision is older than
    /// the newest one seen is dropped.
    pub fn bind_rich_editor(self, editor: RichEditorElement) -> RichEditorElement {
        let reference = component_ref::<RichEditor>();
        let newest = Rc::new(Cell::new(0u64));
        editor
            .component_ref(&reference)
            .content(move || self.value())
            .on_change(move |change: &RichEditorChange| {
                if change.revision < newest.get() {
                    return;
                }
                newest.set(change.revision);
                if let Some(api) = reference.api::<RichEditorApi>() {
                    self.handle_change(Some(api.doc_json()));
                }
            })
    }
}
