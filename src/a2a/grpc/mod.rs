//! gRPC module for A2A protocol
//! 
//! This module contains gRPC-related functionality
//! matching a2a-python/src/a2a/grpc/

pub mod a2a_pb2;
pub mod a2a_pb2_grpc;

// Re-export gRPC types
pub use a2a_pb2::a2a;
