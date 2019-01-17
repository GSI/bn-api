#![deny(unreachable_patterns)]
#![deny(unknown_lints)]
#![cfg_attr(not(debug_assertions), deny(unused_variables))]
#![cfg_attr(not(debug_assertions), deny(unused_imports))]
#![cfg_attr(not(debug_assertions), deny(dead_code))]
// Unused results is more often than not an error
#![deny(unused_must_use)]
#![cfg_attr(not(debug_assertions), deny(unused_extern_crates))]
extern crate actix_web;
extern crate bigneon_db;
//#[macro_use]
extern crate chrono;
extern crate diesel;
extern crate dotenv;
extern crate futures;
extern crate globee;
extern crate itertools;
extern crate jsonwebtoken as jwt;
extern crate lettre;
extern crate lettre_email;
#[macro_use]
extern crate log;
#[macro_use]
extern crate logging;
extern crate r2d2;
extern crate regex;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate serde_with;
extern crate stripe;
extern crate tari_client;
extern crate tokio;
extern crate twilio;
extern crate uuid;
extern crate validator;
#[macro_use]
extern crate validator_derive;

pub mod auth;
pub mod communications;
pub mod config;
pub mod controllers;
pub mod db;
pub mod domain_events;
pub mod errors;
pub mod extractors;
pub mod helpers;
pub mod middleware;
pub mod models;
mod payments;
mod routing;
pub mod server;
pub mod utils;
