//! Error types for org-element parsing and manipulation.

use thiserror::Error;

/// Result type alias for org-element operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during Org document parsing and manipulation.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Error parsing the Org document.
    #[error("Parse error at byte {position}: {message}")]
    ParseError {
        /// The byte position where the error occurred.
        position: usize,
        /// Description of the error.
        message: String,
    },

    /// The tree-sitter parser returned an error.
    #[error("Tree-sitter error: {0}")]
    TreeSitterError(String),

    /// Invalid node type encountered.
    #[error("Invalid node type: {0}")]
    InvalidNodeType(String),

    /// Missing required property.
    #[error("Missing required property '{property}' on {node_type}")]
    MissingProperty {
        /// The node type that's missing the property.
        node_type: String,
        /// The name of the missing property.
        property: String,
    },

    /// Invalid property value.
    #[error("Invalid property value for '{property}' on {node_type}: {message}")]
    InvalidProperty {
        /// The node type.
        node_type: String,
        /// The property name.
        property: String,
        /// Description of why it's invalid.
        message: String,
    },

    /// Attempted to insert an object in an invalid context.
    #[error("Object {object_type} is not allowed in {container_type}")]
    ObjectRestriction {
        /// The object type being inserted.
        object_type: String,
        /// The container type.
        container_type: String,
    },

    /// Attempted to perform an invalid tree operation.
    #[error("Invalid tree operation: {0}")]
    InvalidOperation(String),

    /// Node not found in the tree.
    #[error("Node not found")]
    NodeNotFound,

    /// Generic error with custom message.
    #[error("{0}")]
    Custom(String),
}

impl Error {
    /// Create a parse error.
    pub fn parse_error(position: usize, message: impl Into<String>) -> Self {
        Self::ParseError {
            position,
            message: message.into(),
        }
    }

    /// Create a missing property error.
    pub fn missing_property(node_type: impl Into<String>, property: impl Into<String>) -> Self {
        Self::MissingProperty {
            node_type: node_type.into(),
            property: property.into(),
        }
    }

    /// Create an invalid property error.
    pub fn invalid_property(
        node_type: impl Into<String>,
        property: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::InvalidProperty {
            node_type: node_type.into(),
            property: property.into(),
            message: message.into(),
        }
    }

    /// Create an object restriction error.
    pub fn object_restriction(
        object_type: impl Into<String>,
        container_type: impl Into<String>,
    ) -> Self {
        Self::ObjectRestriction {
            object_type: object_type.into(),
            container_type: container_type.into(),
        }
    }
}
