//! Custom types serialized in nushell inside
//! [`Value::Custom`](nu_protocol::Value::Custom)s.

use std::sync::Arc;

use nu_protocol::{CustomValue, ShellError, Span, Value};
use parking_lot::RwLock;
use serde::Serialize;

/// The global [`State`](crate::state::State) as stored in
/// [`QUAKE_VARIABLE_ID`](crate::QUAKE_VARIABLE_ID).
#[derive(Clone, Debug)]
pub struct State(pub Arc<RwLock<crate::state::State>>);

impl CustomValue for State {
    fn clone_value(&self, span: Span) -> Value {
        Value::custom_value(Box::new(self.clone()), span)
    }

    fn value_string(&self) -> String {
        self.typetag_name().to_owned()
    }

    fn to_base_value(&self, span: Span) -> Result<Value, ShellError> {
        // TODO implement this?
        Err(ShellError::GenericError {
            error: "`$quake` cannot be represented as a nushell value".to_owned(),
            msg: String::new(),
            span: Some(span),
            help: None,
            inner: Vec::new(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn typetag_name(&self) -> &'static str {
        "State" // TODO name?
    }

    fn typetag_deserialize(&self) {
        unimplemented!("typetag_deserialize")
    }
}

impl Serialize for State {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        unimplemented!("serialize")
    }
}
