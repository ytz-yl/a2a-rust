//! Utility functions for A2A protocol operations
//! 
//! This module provides helper functions for creating and manipulating A2A objects,
//! matching the functionality provided in a2a-python/src/a2a/utils/.

pub mod artifact;
pub mod constants;
pub mod message;
pub mod parts;
pub mod task;
pub mod proto_utils;

// Re-export utility functions for convenience
pub use artifact::*;
pub use constants::*;

// Re-export message utilities with explicit naming to avoid conflicts
pub use message::{
    new_agent_text_message,
    new_agent_parts_message,
    get_message_text,
};

// Re-export parts utilities with explicit naming to avoid conflicts
pub use parts::{
    get_data_parts,
    get_file_parts,
    get_text_parts as get_parts_text,
};

pub use task::*;
pub use proto_utils::*;
