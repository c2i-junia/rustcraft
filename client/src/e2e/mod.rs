//! End-to-end tests of the client: the whole app runs headless, and the tests drive its screens.

// The harness offers what the tests of the screens need, and not all of it is used yet.
#![allow(dead_code)]

mod capture;
mod harness;
mod input;
mod locator;
mod seed;
