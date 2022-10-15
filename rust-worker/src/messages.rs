use std::collections::HashMap;
use std::error::Error;

use rmpv::Value;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::Error as SerdeDeError;
use serde::ser::Error as SerdeSeError;
use serde_bytes::ByteBuf;
use serde_json::Value as JSONValue;
use uuid::Uuid;

use crate::Worker;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Message {
    IsAlive(IsAliveMessage),
    Enqueue(EnqueueMessage),
    Eval(EvalMessage),
    Err(ErrMessage),
    Heartbeat,
    Registered,
}

impl Message {
    pub fn id(&self) -> Option<&str> {
        match self {
            Message::IsAlive(message) => Some(message.sync.as_str()),
            Message::Enqueue(message) => Some(message.id()),
            Message::Eval(message) => Some(message.id.as_str()),
            Message::Err(message) => Some(message.sync.as_str()),
            Message::Heartbeat => None,
            Message::Registered => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IsAliveMessage {
    #[serde(rename = "__sync")]
    sync: String,
}

impl IsAliveMessage {
    pub fn new() -> Self {
        Self {
            sync: Uuid::new_v4().hyphenated().to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueMessage {
    statements: String,
    command_id: String,
    bindings: HashMap<String, ByteBuf>,
}

impl EnqueueMessage {
    pub fn new_raw(statement: impl Into<String>) -> Self {
        let command_id = Uuid::new_v4().hyphenated().to_string();

        let mut message = Self {
            statements: statement.into(),
            command_id: command_id.clone(),
            bindings: Default::default(),
        };

        message.add_binding("pharoCommandId", &command_id).unwrap();
        message
    }

    pub fn new(statement: impl Into<String>) -> Self {
        let full_statement = format!(
            "llCommand notify: ({}) id: pharoCommandId.",
            statement.into()
        );
        Self::new_raw(full_statement)
    }

    pub fn id(&self) -> &str {
        self.command_id.as_str()
    }

    pub fn add_binding<V>(
        &mut self,
        name: impl Into<String>,
        value: &V,
    ) -> Result<(), Box<dyn Error>>
    where
        V: Serialize + ?Sized,
    {
        let encoded = rmp_serde::to_vec_named(value)?;
        self.bindings.insert(name.into(), ByteBuf::from(encoded));
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EvalMessage {
    pub id: String,
    #[serde(serialize_with = "value_to_bytes")]
    #[serde(deserialize_with = "bytes_to_value")]
    pub value: Value,
}

impl EvalMessage {
    pub fn json_value(&self) -> Result<serde_json::Value, Box<dyn Error>> {
        match &self.value {
            Value::String(string) => match string.as_str() {
                Some(string) => serde_json::from_str(string).map_err(|err| err.into()),
                None => Err(format!(
                    "Eval String could not be converted to UTF-8: {}",
                    &self.value
                )
                .into()),
            },
            _ => Err(format!("Eval value is not a String: {}", &self.value).into()),
        }
    }

    pub fn task_context_id(&self) -> Result<String, Box<dyn Error>> {
        match self.json_value() {
            Ok(json) => match &json {
                JSONValue::Object(object) => match object.get("taskContextId") {
                    Some(id) => match id {
                        JSONValue::String(id) => Ok(id.to_string()),
                        _ => Err(format!("taskContextId is not a String: {}", id).into()),
                    },
                    None => {
                        Err(format!("taskContextId key does not exist in: {:?}", object).into())
                    }
                },
                _ => Err(format!("Eval JSON is not an Object: {}", &json).into()),
            },
            Err(error) => Err(error),
        }
    }
}

fn value_to_bytes<S>(value: &Value, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, value)
        .map_err(|error| S::Error::custom(error.to_string()))?;

    serializer.serialize_bytes(buf.as_slice())
}

fn bytes_to_value<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: Deserializer<'de>,
{
    let bytes: ByteBuf = serde_bytes::deserialize(deserializer)?;
    let value = rmpv::decode::read_value(&mut bytes.as_ref());
    value.map_err(|error| D::Error::custom(error.to_string()))
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ErrMessage {
    err_msg: String,
    trace: String,
    command_id: String,
    #[serde(rename = "__sync")]
    sync: String,
}

pub fn is_alive_message() -> Result<Message, Box<dyn Error>> {
    Ok(Message::IsAlive(IsAliveMessage::new()))
}

pub fn register_worker_message(worker: &Worker) -> Result<Message, Box<dyn Error>> {
    let statement = "GtAssignedRemoteRunner registerWorkerWithDetails: attributes";

    let mut message = EnqueueMessage::new(statement);
    message.add_binding("attributes", &worker.attributes())?;

    Ok(Message::Enqueue(message))
}

pub fn add_observer_message(worker: &Worker) -> Result<Message, Box<dyn Error>> {
    let statement = "GtAssignedRemoteRunner
			addObserver: [ :anObject |
				PharoLinkAnswerByValue
					value: #value
					during: [ llCommand
							notify: anObject
							command: pharoCommandId
							observer: observerId ] ]
			toWorkerId: workerId";

    let observer_id = Uuid::new_v4().hyphenated().to_string();

    let mut message = EnqueueMessage::new(statement);
    message.add_binding("workerId", &worker.id().to_string())?;
    message.add_binding("observerId", &observer_id)?;

    Ok(Message::Enqueue(message))
}

pub fn next_task_for_worker_message(worker: &Worker) -> Result<Message, Box<dyn Error>> {
    let statement = "GtAssignedRemoteRunner nextTaskSerializedForWorkerId: workerId";

    let mut message = EnqueueMessage::new(statement);
    message.add_binding("workerId", &worker.id().to_string())?;

    Ok(Message::Enqueue(message))
}

pub fn task_result_message(task_context_id: String) -> Result<Message, Box<dyn Error>> {
    let statement = "GtAssignedRemoteRunner
	    taskDone: taskContextId
	    executionData: (LeJsonV4 uniqueInstance deserialize: executionData readStream)
	    result: (LeJsonV4 uniqueInstance deserialize: result readStream).
    llCommand notify: true id: pharoCommandId.";

    let mut message = EnqueueMessage::new_raw(statement);
    message.add_binding("executionData", &include_str!("executionData.json"))?;
    message.add_binding("taskContextId", &task_context_id)?;
    message.add_binding("result", serde_json::to_string("result")?.as_str())?;

    Ok(Message::Enqueue(message))
}
