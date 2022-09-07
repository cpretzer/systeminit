use super::ChangeSetResult;
use crate::server::extract::{AccessBuilder, HandlerContext};
use axum::Json;
use dal::{ChangeSet, ChangeSetPk, LabelList};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ListOpenChangeSetsResponse {
    pub list: LabelList<ChangeSetPk>,
}

pub async fn list_open_change_sets(
    HandlerContext(builder): HandlerContext,
    AccessBuilder(request_ctx): AccessBuilder,
) -> ChangeSetResult<Json<ListOpenChangeSetsResponse>> {
    let ctx = builder.build(request_ctx.build_head()).await?;

    let list = ChangeSet::list_open(&ctx).await?;

    Ok(Json(ListOpenChangeSetsResponse { list }))
}
