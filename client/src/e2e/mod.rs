//! End-to-end tests of the client: the whole app runs headless, and the tests drive its screens.

// The harness offers what the tests of the screens need, and not all of it is used yet
// (`right_click`, `row`, `within`, `text_of`, `snapshot`), and the watchdog is only held for its
// drop.
#![allow(dead_code)]

mod capture;
mod harness;
mod helpers;
mod input;
mod locator;
mod main_menu;
mod multi;
mod seed;
mod solo;
