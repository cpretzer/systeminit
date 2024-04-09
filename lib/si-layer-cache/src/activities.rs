use std::{
    collections::HashMap,
    fmt::{self, Debug},
    str::FromStr,
    sync::Arc,
};

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use si_data_nats::{
    async_nats::jetstream::{self, Message},
    jetstream::Stream,
    NatsClient,
};
use strum::EnumDiscriminants;
use tokio::sync::{broadcast, mpsc::UnboundedReceiver, mpsc::UnboundedSender, RwLock};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use ulid::{Ulid, ULID_LEN};

use crate::{
    error::LayerDbResult,
    event::LayeredEventMetadata,
    nats::{self, subject},
    LayerDbError,
};

use self::{
    rebase::{RebaseFinished, RebaseRequest},
    test::{IntegrationTest, IntegrationTestAlt},
};

use telemetry::prelude::*;

pub use si_data_nats::async_nats::jetstream::AckKind;

pub mod rebase;
pub mod test;

#[derive(Copy, Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ActivityId(Ulid);

impl ActivityId {
    pub fn new() -> ActivityId {
        ActivityId(Ulid::new())
    }

    pub fn array_to_str<'buf>(&self, buf: &'buf mut [u8; ULID_LEN]) -> &'buf mut str {
        self.0.array_to_str(buf)
    }

    pub fn into_inner(self) -> Ulid {
        self.0
    }
}

impl Default for ActivityId {
    fn default() -> Self {
        Self::new()
    }
}

impl FromStr for ActivityId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Ulid::from_str(s)?))
    }
}

impl From<ulid::Ulid> for ActivityId {
    fn from(value: ulid::Ulid) -> Self {
        Self(value)
    }
}

impl fmt::Display for ActivityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Activity {
    pub id: ActivityId,
    pub payload: ActivityPayload,
    pub metadata: LayeredEventMetadata,
    pub parent_activity_id: Option<ActivityId>,
}

impl Activity {
    pub fn new(
        payload: ActivityPayload,
        metadata: LayeredEventMetadata,
        parent_activity_id: Option<ActivityId>,
    ) -> Activity {
        Activity {
            id: ActivityId::new(),
            payload,
            metadata,
            parent_activity_id,
        }
    }

    pub fn rebase(request: RebaseRequest, metadata: LayeredEventMetadata) -> Activity {
        Activity::new(ActivityPayload::RebaseRequest(request), metadata, None)
    }

    pub fn rebase_finished(
        request: RebaseFinished,
        metadata: LayeredEventMetadata,
        from_rebase_activity_id: ActivityId,
    ) -> Activity {
        Activity::new(
            ActivityPayload::RebaseFinished(request),
            metadata,
            Some(from_rebase_activity_id),
        )
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, EnumDiscriminants, PartialEq, Eq)]
pub enum ActivityPayload {
    RebaseRequest(RebaseRequest),
    RebaseFinished(RebaseFinished),
    IntegrationTest(IntegrationTest),
    IntegrationTestAlt(IntegrationTestAlt),
}

impl ActivityPayload {
    pub fn to_subject(&self) -> String {
        let discriminate: ActivityPayloadDiscriminants = self.into();
        discriminate.to_subject()
    }
}

impl ActivityPayloadDiscriminants {
    pub fn to_subject(&self) -> String {
        match self {
            ActivityPayloadDiscriminants::RebaseRequest => "rebase.request".to_string(),
            ActivityPayloadDiscriminants::RebaseFinished => "rebase.finished".to_string(),
            ActivityPayloadDiscriminants::IntegrationTest => "integration_test.test".to_string(),
            ActivityPayloadDiscriminants::IntegrationTestAlt => "integration_test.alt".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActivityPublisher {
    prefix: Option<Arc<str>>,
    context: jetstream::context::Context,
}

impl ActivityPublisher {
    pub(crate) fn new(nats_client: &NatsClient) -> ActivityPublisher {
        let prefix = nats_client.metadata().subject_prefix().map(|s| s.into());
        let context = jetstream::new(nats_client.as_inner().clone());
        ActivityPublisher { context, prefix }
    }

    pub(crate) async fn publish(&self, activity: &Activity) -> LayerDbResult<()> {
        let nats_subject = subject::for_activity(self.prefix(), activity);
        let nats_payload = postcard::to_stdvec(&activity)?;
        // Publish message and await confirmation from server that it has been received
        self.context
            .publish(nats_subject, nats_payload.into())
            .await?
            .await?;
        Ok(())
    }

    fn prefix(&self) -> Option<&str> {
        self.prefix.as_deref()
    }
}

#[derive(Debug, Clone)]
pub struct ActivityMultiplexer {
    instance_id: Ulid,
    nats_client: NatsClient,
    tracker: TaskTracker,
    shutdown_token: CancellationToken,
    channels: Arc<RwLock<HashMap<String, broadcast::Sender<Activity>>>>,
}

impl ActivityMultiplexer {
    pub fn new(
        instance_id: Ulid,
        nats_client: NatsClient,
        shutdown_token: CancellationToken,
    ) -> Self {
        let tracker = TaskTracker::new();
        Self {
            tracker,
            instance_id,
            nats_client,
            shutdown_token,
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn subscribe(
        &self,
        filters: Option<impl IntoIterator<Item = ActivityPayloadDiscriminants>>,
    ) -> LayerDbResult<broadcast::Receiver<Activity>> {
        let (multiplex_key, has_filter_array) = if let Some(filters) = filters {
            let filter_array: Vec<ActivityPayloadDiscriminants> = filters.into_iter().collect();
            (
                filter_array
                    .iter()
                    .map(|d| d.to_subject())
                    .collect::<Vec<String>>()
                    .join("."),
                Some(filter_array),
            )
        } else {
            ("everything".to_string(), None)
        };
        {
            let reader = self.channels.read().await;
            if let Some(sender) = reader.get(&multiplex_key) {
                return Ok(sender.subscribe());
            }
        }
        let activity_stream = ActivityStream::run(
            self.instance_id,
            &self.nats_client,
            self.tracker.clone(),
            has_filter_array,
        )
        .await?;
        let (tx, rx) = broadcast::channel(1_000_000); // the 1_000_000 here is the depth the channel will
                                                      // keep if a reader is slow
        let mut amx_task = ActivityMultiplexerTask::new(activity_stream, tx.clone());
        let amx_shutdown_token = self.shutdown_token.clone();
        self.tracker
            .spawn(async move { amx_task.run(amx_shutdown_token).await });
        {
            let mut writer = self.channels.write().await;
            writer.insert(multiplex_key, tx);
        }
        Ok(rx)
    }
}

pub struct ActivityMultiplexerTask {
    activity_stream: UnboundedReceiver<Activity>,
    tx: broadcast::Sender<Activity>,
}

impl ActivityMultiplexerTask {
    pub fn new(
        activity_stream: UnboundedReceiver<Activity>,
        tx: broadcast::Sender<Activity>,
    ) -> Self {
        Self {
            activity_stream,
            tx,
        }
    }

    pub async fn run(&mut self, token: CancellationToken) -> LayerDbResult<()> {
        tokio::select! {
            () = self.process() => {
                debug!("activity multiplexer task has ended; likely a bug");
            },
            () = token.cancelled() => {
                debug!("activity multiplexer has been cancelled; shutting down");
            },
        }
        Ok(())
    }

    pub async fn process(&mut self) {
        while let Some(activity) = self.activity_stream.recv().await {
            if let Err(error) = self.tx.send(activity) {
                trace!(
                    ?error,
                    "activity multiplexer skipping message; no receivers listening. this can be totally normal!"
                );
            }
        }
    }
}

pub struct ActivityStream;

impl ActivityStream {
    pub(crate) async fn run(
        instance_id: Ulid,
        nats_client: &NatsClient,
        tracker: TaskTracker,
        filters: Option<impl IntoIterator<Item = ActivityPayloadDiscriminants>>,
    ) -> LayerDbResult<UnboundedReceiver<Activity>> {
        let context = jetstream::new(nats_client.as_inner().clone());

        let stream =
            nats::layerdb_activities_stream(&context, nats_client.metadata().subject_prefix())
                .await?
                .create_consumer(Self::consumer_config(
                    nats_client.metadata().subject_prefix(),
                    instance_id,
                    filters,
                ))
                .await?
                .messages()
                .await?;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let spawn_tracker = tracker.clone();
        tracker.spawn(async move { Self::process_messages(stream, spawn_tracker, tx).await });

        Ok(rx)
    }

    pub async fn process_messages(
        mut stream: Stream,
        tracker: TaskTracker,
        tx: UnboundedSender<Activity>,
    ) {
        while let Some(msg_result) = stream.next().await {
            match msg_result {
                Ok(msg) => {
                    if let Err(error) = msg.ack().await {
                        warn!(
                            ?error,
                            "Error sending message ack while processing activity stream"
                        );
                    }
                    let tx = tx.clone();
                    tracker.spawn(async move {
                        if let Err(error) = Self::process_message(tx, msg).await {
                            warn!(?error, "error processing message in activity stream");
                        }
                    });
                }
                Err(error) => {
                    warn!(?error, "Error processing activity stream");
                }
            }
        }
    }

    pub async fn process_message(tx: UnboundedSender<Activity>, msg: Message) -> LayerDbResult<()> {
        let activity = postcard::from_bytes::<Activity>(&msg.payload)?;
        tx.send(activity).map_err(Box::new)?;
        Ok(())
    }

    #[inline]
    fn consumer_config(
        prefix: Option<&str>,
        instance_id: Ulid,
        filters: Option<impl IntoIterator<Item = ActivityPayloadDiscriminants>>,
    ) -> jetstream::consumer::pull::Config {
        let name = format!("activity-stream-{instance_id}");
        let description = format!("activity stream for [{name}]");

        let mut config = jetstream::consumer::pull::Config {
            name: Some(name),
            description: Some(description),
            deliver_policy: jetstream::consumer::DeliverPolicy::New,
            ..Default::default()
        };

        if let Some(payload_types) = filters {
            config.filter_subjects = payload_types
                .into_iter()
                .map(|t| nats::subject::for_activity_discriminate(prefix, t))
                .map(|s| s.to_string())
                .collect();
        }

        config
    }
}

#[derive(Clone, Debug)]
pub struct ActivityRebaseRequest {
    pub id: ActivityId,
    pub payload: RebaseRequest,
    pub metadata: LayeredEventMetadata,
    pub parent_activity_id: Option<ActivityId>,
}

impl TryFrom<Activity> for ActivityRebaseRequest {
    type Error = LayerDbError;

    fn try_from(activity: Activity) -> LayerDbResult<Self> {
        let payload = match activity.payload {
            ActivityPayload::RebaseRequest(payload) => payload,
            _ => return Err(LayerDbError::ActivityRebase),
        };
        Ok(ActivityRebaseRequest {
            id: activity.id,
            payload,
            metadata: activity.metadata,
            parent_activity_id: activity.parent_activity_id,
        })
    }
}

pub struct RebaserRequestWorkQueue {
    tracker: TaskTracker,
    shutdown_token: CancellationToken,
    stream: Stream,
    tx: tokio::sync::mpsc::UnboundedSender<ActivityRebaseRequest>,
}

impl RebaserRequestWorkQueue {
    const NAME: &'static str = "LayerDB::RebaserRequestWorkQueue";
    const CONSUMER_NAME: &'static str = "rebaser-requests";

    pub async fn create(
        nats_client: NatsClient,
        shutdown_token: CancellationToken,
    ) -> LayerDbResult<(Self, UnboundedReceiver<ActivityRebaseRequest>)> {
        let tracker = TaskTracker::new();
        let context = jetstream::new(nats_client.as_inner().clone());

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        // Ensure the sourced stream is created
        let _activities =
            nats::layerdb_activities_stream(&context, nats_client.metadata().subject_prefix())
                .await?;

        let stream = nats::rebaser_requests_work_queue_stream(
            &context,
            nats_client.metadata().subject_prefix(),
        )
        .await?
        .create_consumer(Self::consumer_config())
        .await?
        .messages()
        .await?;

        Ok((
            Self {
                tracker,
                shutdown_token,
                stream,
                tx,
            },
            rx,
        ))
    }

    pub async fn run(&mut self) {
        let shutdown_token = self.shutdown_token.clone();
        tokio::select! {
            _ = self.process_messages() => {
            }
            _ = shutdown_token.cancelled() => {
                debug!(task = Self::NAME, "received cancellation");
            }
        }

        // All remaining work has been dispatched (i.e. spawned) so no more tasks will be spawned
        self.tracker.close();
        // Wait for all in-flight writes work to complete
        self.tracker.wait().await;

        debug!(task = Self::NAME, "shutdown complete");
    }

    pub async fn process_messages(&mut self) {
        while let Some(msg_result) = self.stream.next().await {
            match msg_result {
                Ok(msg) => {
                    if let Err(error) = msg.ack().await {
                        warn!(?error, "rebaser request work queue message ack error");
                    }
                    if let Err(error) = self.process_message(msg).await {
                        error!(?error, "rebaser request has failed to process message");
                    }
                }
                Err(error) => {
                    warn!(?error, "rebaser request work queue has failed to get a message from the nats stream");
                }
            }
        }
    }

    pub async fn process_message(&self, msg: Message) -> LayerDbResult<()> {
        let activity = postcard::from_bytes::<Activity>(&msg.payload)?;
        let rebase_activity = activity.try_into()?;
        self.tx.send(rebase_activity).map_err(Box::new)?;
        Ok(())
    }

    #[inline]
    fn consumer_config() -> jetstream::consumer::pull::Config {
        jetstream::consumer::pull::Config {
            durable_name: Some(Self::CONSUMER_NAME.to_string()),
            description: Some("rebaser requests consumer".to_string()),
            ..Default::default()
        }
    }
}