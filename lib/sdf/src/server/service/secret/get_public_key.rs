use axum::Json;
use dal::PublicKey;

use super::SecretResult;
use crate::server::extract::{AccessBuilder, Authorization, HandlerContext};

pub type GetPublicKeyResponse = PublicKey;

pub async fn get_public_key(
    HandlerContext(builder): HandlerContext,
    AccessBuilder(request_ctx): AccessBuilder,
    Authorization(claim): Authorization,
) -> SecretResult<Json<GetPublicKeyResponse>> {
    let ctx = builder.build(request_ctx.build_head()).await?;

    let response: GetPublicKeyResponse = PublicKey::get_current(
        &ctx,
        &claim.find_billing_account_pk_for_workspace(&ctx).await?,
    )
    .await?;

    Ok(Json(response))
}
