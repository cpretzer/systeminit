use crate::dal::test;
use dal::{
    property_editor::{PropertyEditorSchema, PropertyEditorValues},
    AttributeReadContext, DalContext, Schema, StandardModel,
};

#[test]
async fn schema_for_schema_variant(ctx: &DalContext<'_, '_>) {
    let schema = Schema::find_by_attr(ctx, "name", &"docker_image".to_string())
        .await
        .expect("cannot find docker image schema")
        .pop()
        .expect("no docker image schema found");
    let schema_variant_id = schema
        .default_schema_variant_id()
        .expect("missing default schema variant id");
    let _property_editor_schema = PropertyEditorSchema::for_schema_variant(ctx, *schema_variant_id)
        .await
        .expect("cannot create property editor schema from schema variant");
    // NOTE: Some day, this test should.. test something. For now, though - we'll do it live.
}

#[test]
async fn value_for_context(ctx: &DalContext<'_, '_>) {
    let schema = Schema::find_by_attr(ctx, "name", &"docker_image".to_string())
        .await
        .expect("cannot find docker image schema")
        .pop()
        .expect("no docker image schema found");
    let schema_variant_id = schema
        .default_schema_variant_id()
        .expect("missing default schema variant id");
    let context = AttributeReadContext {
        schema_id: Some(*schema.id()),
        schema_variant_id: Some(*schema_variant_id),
        prop_id: None,
        ..AttributeReadContext::default()
    };
    let _property_editor_values = PropertyEditorValues::for_context(ctx, context)
        .await
        .expect("cannot create property editor values from context");
    // NOTE: Some day, this test should.. test something. For now, though - we'll do it live.
}
