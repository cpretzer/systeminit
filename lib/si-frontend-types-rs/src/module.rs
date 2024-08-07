use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use si_events::SchemaVariantId;

pub use module_index_types::LatestModuleResponse as LatestModule;

#[derive(Clone, Debug, Deserialize, Eq, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncedModules {
    pub upgradeable: HashMap<SchemaVariantId, LatestModule>,
    pub installable: Vec<LatestModule>,
}

impl SyncedModules {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModuleContributeRequest {
    pub name: String,
    pub version: String,
    pub schema_variant_id: SchemaVariantId,
}
