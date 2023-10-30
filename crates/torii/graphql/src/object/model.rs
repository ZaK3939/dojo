use async_graphql::dynamic::indexmap::IndexMap;
use async_graphql::dynamic::{InputValue, SubscriptionField, SubscriptionFieldFuture, TypeRef};
use async_graphql::{Name, Value};
use tokio_stream::StreamExt;
use torii_core::simple_broker::SimpleBroker;
use torii_core::types::Model;

use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::mapping::MODEL_TYPE_MAPPING;
use crate::query::constants::MODEL_TABLE;

pub struct ModelObject;

// TODO: Refactor subscription to not use this
impl ModelObject {
    pub fn value_mapping(model: Model) -> ValueMapping {
        IndexMap::from([
            (Name::new("id"), Value::from(model.id)),
            (Name::new("name"), Value::from(model.name)),
            (Name::new("classHash"), Value::from(model.class_hash)),
            (Name::new("transactionHash"), Value::from(model.transaction_hash)),
            (
                Name::new("createdAt"),
                Value::from(model.created_at.format("%Y-%m-%d %H:%M:%S").to_string()),
            ),
        ])
    }
}

impl ObjectTrait for ModelObject {
    fn name(&self) -> (&str, &str) {
        ("model", "models")
    }

    fn type_name(&self) -> &str {
        "World__Model"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &MODEL_TYPE_MAPPING
    }

    fn table_name(&self) -> Option<&str> {
        Some(MODEL_TABLE)
    }

    fn subscriptions(&self) -> Option<Vec<SubscriptionField>> {
        let name = format!("{}Registered", self.name().0);
        Some(vec![
            SubscriptionField::new(name, TypeRef::named_nn(self.type_name()), |ctx| {
                {
                    SubscriptionFieldFuture::new(async move {
                        let id = match ctx.args.get("id") {
                            Some(id) => Some(id.string()?.to_string()),
                            None => None,
                        };
                        // if id is None, then subscribe to all models
                        // if id is Some, then subscribe to only the model with that id
                        Ok(SimpleBroker::<Model>::subscribe().filter_map(move |model: Model| {
                            if id.is_none() || id == Some(model.id.clone()) {
                                Some(Ok(Value::Object(ModelObject::value_mapping(model))))
                            } else {
                                // id != model.id, so don't send anything, still listening
                                None
                            }
                        }))
                    })
                }
            })
            .argument(InputValue::new("id", TypeRef::named(TypeRef::ID))),
        ])
    }
}