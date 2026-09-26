mod agent;
mod agent_definition;
mod context_engine;
mod conversation;
mod database;
mod peach_repo;
mod fs_snap;
mod fuzzy_search;
mod provider;
mod skill;
mod thread_event;
mod validation;

mod proto_generated {
    tonic::include_proto!("peach.v1");
}

// Only expose peach_repo container
pub use peach_repo::*;
pub use thread_event::ThreadEventRepositoryImpl;
