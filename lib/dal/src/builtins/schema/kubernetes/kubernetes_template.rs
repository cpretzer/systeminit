use crate::builtins::schema::kubernetes::kubernetes_metadata::create_metadata_prop;
use crate::builtins::schema::kubernetes::kubernetes_spec::create_spec_prop;
use crate::builtins::schema::BuiltinSchemaHelpers;
use crate::{builtins::BuiltinsResult, DalContext, Prop, PropId, PropKind, StandardModel};

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub async fn create_template_prop(
    ctx: &DalContext,
    parent_prop_id: Option<PropId>,
) -> BuiltinsResult<Prop> {
    let template_prop =
        BuiltinSchemaHelpers::create_prop(ctx, "template", PropKind::Object, parent_prop_id, None)
            .await?;

    {
        let _optional_metadata_prop = create_metadata_prop(ctx, false, *template_prop.id()).await?;
    }

    {
        let _spec_prop = create_spec_prop(ctx, *template_prop.id()).await?;
    }
    Ok(template_prop)
}
