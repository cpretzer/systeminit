WITH RECURSIVE props_tree
                   AS
                   ((SELECT DISTINCT ON
                       (props.id) row_to_json(props.*) AS object,
                                  props.id             AS root_id,
                                  props.id             AS prop_id,
                                  '/' || props.name    AS path,
                                  -1::bigint           AS parent_id,
                                  0::bigint            AS depth
                     FROM props
                              LEFT JOIN prop_belongs_to_prop pbtp on props.id = pbtp.object_id
                     WHERE pbtp.belongs_to_id IS NULL
                       AND in_tenancy_and_visible_v1($1, $2, props)
                     ORDER BY props.id, props.visibility_change_set_pk)

                    UNION ALL

                    (SELECT DISTINCT ON
                        (child_props.id) row_to_json(child_props.*)             AS object,
                                         parent.root_id                         AS root_id,
                                         child_props.id                         AS prop_id,
                                         parent.path || '/' || child_props.name AS path,
                                         parent.prop_id                         AS parent_id,
                                         parent.depth + 1                       AS depth
                     FROM props child_props
                              JOIN prop_belongs_to_prop pbtp2 on child_props.id = pbtp2.object_id
                              JOIN props_tree AS parent
                                   ON parent.prop_id = pbtp2.belongs_to_id
                     WHERE in_tenancy_and_visible_v1($1, $2, child_props)
                     ORDER BY child_props.id, child_props.visibility_change_set_pk))
SELECT pmtmsv.right_object_id AS schema_variant_id,
       props_tree.object,
       props_tree.root_id,
       props_tree.prop_id,
       props_tree.parent_id,
       props_tree.path
FROM props_tree
         JOIN prop_many_to_many_schema_variants pmtmsv
              ON pmtmsv.left_object_id = props_tree.root_id
ORDER BY schema_variant_id, root_id, depth;