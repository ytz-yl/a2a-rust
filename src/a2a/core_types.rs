//! Core type definitions for the A2A protocol
//! 
//! This module contains all the fundamental types used throughout the A2A protocol,
//! including enums, basic structures, and common data types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use url::Url;
use uuid::Uuid;

/// The location of the API key for authentication
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum In {
    Cookie,
    Header,
    Query,
}

/// Identifies the sender of a message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Agent,
}

/// Defines the lifecycle states of a Task
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskState {
    Submitted,
    Working,
    InputRequired,
    Completed,
    Canceled,
    Failed,
    Rejected,
    AuthRequired,
    Unknown,
}

/// Supported A2A transport protocols
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TransportProtocol {
    Jsonrpc,
    Grpc,
    HttpJson,
}

impl fmt::Display for TransportProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportProtocol::Jsonrpc => write!(f, "JSONRPC"),
            TransportProtocol::Grpc => write!(f, "GRPC"),
            TransportProtocol::HttpJson => write!(f, "HTTP_JSON"),
        }
    }
}

impl FromStr for TransportProtocol {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "JSONRPC" => Ok(TransportProtocol::Jsonrpc),
            "GRPC" => Ok(TransportProtocol::Grpc),
            "HTTP_JSON" => Ok(TransportProtocol::HttpJson),
            _ => Err(format!("Unknown transport protocol: {}", s)),
        }
    }
}

/// Represents a structured data segment (e.g., JSON) within a message or artifact
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataPart {
    /// The structured data content
    pub data: serde_json::Value,
    /// The type of this part, used as a discriminator. Always 'data'
    #[serde(rename = "kind")]
    pub kind: String,
    /// Optional metadata associated with this part
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl DataPart {
    pub fn new(data: serde_json::Value) -> Self {
        Self {
            data,
            kind: "data".to_string(),
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Represents a text segment within a message or artifact
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextPart {
    /// The string content of the text part
    pub text: String,
    /// The type of this part, used as a discriminator. Always 'text'
    #[serde(rename = "kind")]
    pub kind: String,
    /// Optional metadata associated with this part
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl TextPart {
    pub fn new(text: String) -> Self {
        Self {
            text,
            kind: "text".to_string(),
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Defines base properties for a file
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileBase {
    /// The MIME type of the file (e.g., "application/pdf")
    #[serde(rename = "mime_type")]
    pub mime_type: Option<String>,
    /// An optional name for the file (e.g., "document.pdf")
    pub name: Option<String>,
}

/// Represents a file with its content provided directly as a base64-encoded string
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileWithBytes {
    /// The base64-encoded content of the file
    pub bytes: String,
    /// The MIME type of the file (e.g., "application/pdf")
    #[serde(rename = "mime_type")]
    pub mime_type: Option<String>,
    /// An optional name for the file (e.g., "document.pdf")
    pub name: Option<String>,
}

/// Represents a file with its content located at a specific URI
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileWithUri {
    /// A URL pointing to the file's content
    pub uri: String, // Changed from Url to String to match Python's str type
    /// The MIME type of the file (e.g., "application/pdf")
    #[serde(rename = "mime_type")]
    pub mime_type: Option<String>,
    /// An optional name for the file (e.g., "document.pdf")
    pub name: Option<String>,
}

/// Represents a file segment within a message or artifact
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilePart {
    /// The file content, represented as either a URI or as base64-encoded bytes
    pub file: FileContent,
    /// The type of this part, used as a discriminator. Always 'file'
    #[serde(rename = "kind")]
    pub kind: String,
    /// Optional metadata associated with this part
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Represents different ways to provide file content
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FileContent {
    Uri(FileWithUri),
    Bytes(FileWithBytes),
}

impl FilePart {
    pub fn new_uri(uri: Url) -> Self {
        Self {
            file: FileContent::Uri(FileWithUri {
                uri: uri.to_string(),
                mime_type: None,
                name: None,
            }),
            kind: "file".to_string(),
            metadata: None,
        }
    }

    pub fn new_bytes(bytes: String) -> Self {
        Self {
            file: FileContent::Bytes(FileWithBytes {
                bytes,
                mime_type: None,
                name: None,
            }),
            kind: "file".to_string(),
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Root part types that can be wrapped in a Part
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PartRoot {
    Text(TextPart),
    File(FilePart),
    Data(DataPart),
}

/// A discriminated union representing a part of a message or artifact
/// This matches Python's Part(RootModel[TextPart | FilePart | DataPart])
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)] // Allow both {"root": {...}} and direct {...} formats
pub enum Part {
    WithRoot { root: PartRoot },
    Direct(PartRoot),
}

impl Part {
    pub fn text(text: String) -> Self {
        Self::Direct(PartRoot::Text(TextPart::new(text)))
    }

    pub fn file_uri(uri: Url) -> Self {
        Self::Direct(PartRoot::File(FilePart::new_uri(uri)))
    }

    pub fn file_bytes(bytes: String) -> Self {
        Self::Direct(PartRoot::File(FilePart::new_bytes(bytes)))
    }

    pub fn data(data: serde_json::Value) -> Self {
        Self::Direct(PartRoot::Data(DataPart::new(data)))
    }

    /// Get the root part (for compatibility with Python's Part.root pattern)
    pub fn root(&self) -> &PartRoot {
        match self {
            Part::WithRoot { root } => root,
            Part::Direct(root) => root,
        }
    }

    /// Custom deserialization to handle both {"root": {...}} and direct {...} formats
    pub fn deserialize_for_compatibility<'de, D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct PartVisitor;

        impl<'de> Visitor<'de> for PartVisitor {
            type Value = Part;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a part object with either 'root' field or direct content")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Part, M::Error>
            where
                M: MapAccess<'de>,
            {
                // Check if the first key is "root"
                if let Some(key) = map.next_key()? {
                    if key == "root" {
                        let root: PartRoot = map.next_value()?;
                        // Ensure there are no more fields
                        if map.next_key::<String>()?.is_some() {
                            return Err(de::Error::custom("unexpected additional fields after 'root'"));
                        }
                        Ok(Part::WithRoot { root })
                    } else {
                        // If the first key is not "root", treat it as a direct part
                        // We need to reconstruct the part from the remaining data
                        let mut value_map = serde_json::Map::new();
                        value_map.insert(key, map.next_value()?);
                        
                        // Add remaining fields
                        while let Some(k) = map.next_key()? {
                            value_map.insert(k, map.next_value()?);
                        }
                        
                        let value = serde_json::Value::Object(value_map);
                        let root: PartRoot = serde_json::from_value(value)
                            .map_err(|e| de::Error::custom(format!("failed to parse part: {}", e)))?;
                        Ok(Part::Direct(root))
                    }
                } else {
                    Err(de::Error::custom("empty object"))
                }
            }
        }

        deserializer.deserialize_any(PartVisitor)
    }
}


impl From<TextPart> for Part {
    fn from(text_part: TextPart) -> Self {
        Self::Direct(PartRoot::Text(text_part))
    }
}

impl From<FilePart> for Part {
    fn from(file_part: FilePart) -> Self {
        Self::Direct(PartRoot::File(file_part))
    }
}

impl From<DataPart> for Part {
    fn from(data_part: DataPart) -> Self {
        Self::Direct(PartRoot::Data(data_part))
    }
}

/// Defines base properties common to all message or artifact parts
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartBase {
    /// Optional metadata associated with this part
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Type alias for metadata (key-value pairs)
pub type Metadata = HashMap<String, serde_json::Value>;

/// Represents the status of a task at a specific point in time
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskStatus {
    /// The current state of the task's lifecycle
    pub state: TaskState,
    /// An optional, human-readable message providing more details about the current status
    pub message: Option<Box<Message>>,
    /// An ISO 8601 datetime string indicating when this status was recorded
    pub timestamp: Option<String>,
}

impl TaskStatus {
    pub fn new(state: TaskState) -> Self {
        Self {
            state,
            message: None,
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
        }
    }

    pub fn with_message(mut self, message: Message) -> Self {
        self.message = Some(Box::new(message));
        self
    }

    pub fn with_timestamp(mut self, timestamp: String) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

// Forward declaration for Message
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// A unique identifier for the message, typically a UUID, generated by the sender
    pub message_id: String,
    /// The context ID for this message, used to group related interactions
    pub context_id: Option<String>,
    /// The ID of the task this message is part of
    pub task_id: Option<String>,
    /// Identifies the sender of the message
    pub role: Role,
    /// An array of content parts that form the message body
    pub parts: Vec<Part>,
    /// Optional metadata for extensions
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    /// The URIs of extensions that are relevant to this message
    pub extensions: Option<Vec<String>>,
    /// A list of other task IDs that this message references for additional context
    pub reference_task_ids: Option<Vec<String>>,
    /// The type of this object, used as a discriminator. Always 'message'
    pub kind: String,
}

impl Message {
    pub fn new(role: Role, parts: Vec<Part>) -> Self {
        Self {
            message_id: Uuid::new_v4().to_string(),
            context_id: None,
            task_id: None,
            role,
            parts,
            metadata: None,
            extensions: None,
            reference_task_ids: None,
            kind: "message".to_string(),
        }
    }

    pub fn with_message_id(mut self, message_id: String) -> Self {
        self.message_id = message_id;
        self
    }

    pub fn with_context_id(mut self, context_id: String) -> Self {
        self.context_id = Some(context_id);
        self
    }

    pub fn with_task_id(mut self, task_id: String) -> Self {
        self.task_id = Some(task_id);
        self
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}
