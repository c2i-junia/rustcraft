//! End-to-end tests of the client: the whole app runs headless, and the tests drive its screens.

// The harness offers what the tests of the screens need, and not all of it is used yet
// (`with_items`, `with_servers`, `right_click`, `row`, `within`, `text_of`, `snapshot`).
#![allow(dead_code)]

mod capture;
mod harness;
mod input;
mod locator;
mod main_menu;
mod seed;
mod solo;
