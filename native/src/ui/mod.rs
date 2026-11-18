//! The app's UI library: one wrapper per `nsh_base` component in use,
//! styled with the theme's tokens only, so screens import controls from
//! `ui` and never from `nsh_base` directly.
//!
//! NSH UI rules (bind every area):
//! - Screens import controls from this module, never from `nsh/base` or
//!   `nsh_base`.
//! - No raw hex, rgb or hsl colors outside `themes/theme.json`; a missing
//!   color is a new token there, read through [`ui::tokens`].
//! - Every part of a base component is styled, including pressed, focused
//!   and disabled states.
//! - Text containers grow with the user's text size: min-heights and
//!   padding, never fixed heights.
//! - Popups paint their own background, border and shadow, and a backdrop
//!   for phones.

pub mod avatar;
pub mod button;
pub mod carousel;
pub mod collapse;
pub mod color;
pub mod command;
pub mod controls;
pub mod desktop;
pub mod dialog;
pub mod dock;
pub mod editor;
pub mod field;
pub mod gallery;
pub mod icons;
pub mod layout;
pub mod link;
pub mod native;
pub mod native_kit;
pub mod native_vs_app;
pub mod nav;
pub mod number;
pub mod pagination;
pub mod pickers;
pub mod popups;
pub mod resizable;
pub mod scrollbar;
pub mod select;
pub mod sheet;
pub mod tab_bar;
pub mod table;
pub mod tabs;
pub mod text;
pub mod toast;
pub mod tokens;
pub mod tree;
