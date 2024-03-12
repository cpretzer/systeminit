use si_layer_cache::layer_cache::LayerCache;

async fn make_layer_cache(db_name: &str) -> LayerCache<&'static str, String> {
    let tempdir = tempfile::TempDir::new_in("/tmp").expect("cannot create tempdir");
    let db = sled::open(tempdir).expect("unable to open sled database");

    LayerCache::new("test1", db, super::setup_pg_db(db_name).await)
        .await
        .expect("cannot create layer cache")
}

#[tokio::test]
async fn empty_insert_and_get() {
    let layer_cache = make_layer_cache("empty_insert_and_get").await;

    layer_cache
        .insert("skid row", "slave to the grind".into())
        .await;

    let skid_row = "skid row";

    // Confirm the insert went into the memory cache
    let memory_result = layer_cache
        .memory_cache()
        .get(&skid_row)
        .await
        .expect("cannot find value in memory cache");
    assert_eq!("slave to the grind", &memory_result[..]);

    // Confirm we can get directly from the layer cache
    let result = layer_cache
        .get(&skid_row)
        .await
        .expect("error finding object")
        .expect("cannot find object in cache");

    assert_eq!("slave to the grind", &result[..]);
}

#[tokio::test]
async fn not_in_memory_but_on_disk_insert() {
    let layer_cache = make_layer_cache("not_in_memory_but_on_disk_insert").await;

    let skid_row = "skid row";

    // Insert the object directly to disk cache
    layer_cache
        .disk_cache()
        .insert("skid row", "slave to the grind".as_bytes())
        .expect("failed to insert to disk cache");

    // There should not be anything for the key in memory cache
    assert!(!layer_cache.memory_cache().contains(&skid_row));

    // Insert through the layer cache
    layer_cache
        .insert("skid row", "slave to the grind".into())
        .await;

    // There should be an entry in memory now
    assert!(layer_cache.memory_cache().contains(&skid_row));
}

#[tokio::test]
async fn get_inserts_to_memory() {
    let layer_cache = make_layer_cache("get_inserts_to_memory").await;

    let skid_row = "skid row";

    let postcard_serialized = postcard::to_stdvec("slave to the grind").expect("should serialize");

    layer_cache
        .disk_cache()
        .insert("skid row", &postcard_serialized)
        .expect("failed to insert to disk cache");

    assert!(!layer_cache.memory_cache().contains(&skid_row));

    layer_cache
        .get(&skid_row)
        .await
        .expect("error getting object from cache")
        .expect("object not in cachche");

    assert!(layer_cache.memory_cache().contains(&skid_row));
}