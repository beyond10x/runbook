/// The principal every daemon of these tests acts as: `runbook serve --principal` and `Config::principal`.
pub const PRINCIPAL: &str = "agent:test-daemon";

mod stub;
mod state;
mod commands;
mod openapi;
mod http;
mod sse;
mod clock;
mod conformance;
mod ui_embed;
mod ui_embed_review;
mod mcp;
mod cli_client;
mod engine;
mod engine_slow;
mod ui_guard;
mod ui_decide;
mod ui_decide_review;
mod principal;
mod runbooks_view;
mod ui_start;
mod security_start;
mod submit_order;
mod answer_order;
mod default_confinement;
mod default_level;
