//! The realm agent: the one part of the hub with Docker's socket. It runs the hosted worlds in
//! containers it specifies itself, on the requests of the hub over a Unix socket.

pub mod agent;
pub mod config;
pub mod socket;
pub mod spec;
