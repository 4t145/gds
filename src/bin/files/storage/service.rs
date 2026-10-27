use std::{collections::HashMap, time::Instant};

use gpui_kit::*;

use super::*;

struct SharedService(Entity<Service>);
impl Global for SharedService {}

pub(super) struct Observation {
    pub started: Instant,
    pub result: Result<Snapshot, Error>,
}

/// One service per application, shared by every window and tab.
pub struct Service {
    pub snapshot: Snapshot,
    pub error: Option<String>,
    pub discovering: bool,
    busy: HashMap<VolumeId, Vec<std::path::PathBuf>>,
    last_observation: Instant,
    usage: super::usage::UsageCache,
    _watcher: Task<()>,
}

impl Service {
    pub fn shared(cx: &mut App) -> Entity<Self> {
        if let Some(service) = cx.try_global::<SharedService>() {
            return service.0.clone();
        }
        let service = cx.new(Self::new);
        cx.set_global(SharedService(service.clone()));
        service
    }

    fn new(cx: &mut Context<Self>) -> Self {
        let last_observation = Instant::now();
        let (sender, receiver) = async_channel::bounded(16);
        #[cfg(target_os = "linux")]
        let watcher = cx.background_executor().spawn(super::linux::watch(sender));
        #[cfg(not(target_os = "linux"))]
        let watcher = cx.background_executor().spawn(async move {
            let _ = sender
                .send(Observation {
                    started: Instant::now(),
                    result: Err(Error::Unsupported),
                })
                .await;
        });
        let watcher = cx.spawn(async move |this, cx| {
            while let Ok(observation) = receiver.recv().await {
                if this
                    .update(cx, |service, cx| {
                        service.apply(observation, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
            drop(watcher);
        });
        Self {
            snapshot: Snapshot::default(),
            error: None,
            discovering: true,
            busy: HashMap::new(),
            last_observation,
            usage: super::usage::UsageCache::default(),
            _watcher: watcher,
        }
    }

    pub fn is_busy(&self, id: &VolumeId) -> bool {
        self.busy.contains_key(id)
    }

    pub fn capacity(&self, id: &VolumeId) -> Option<&CapacityState> {
        self.usage.get(id)
    }

    pub fn refresh_capacity(&mut self, cx: &mut Context<Self>) {
        self.usage.invalidate();
        self.sample_capacity(cx);
    }

    /// Re-read volume metadata as well as space usage after an explicit refresh.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.discovering = true;
        let started = Instant::now();
        let work = cx.background_executor().spawn(async {
            #[cfg(target_os = "linux")]
            {
                super::linux::refresh().await
            }
            #[cfg(not(target_os = "linux"))]
            {
                Err(Error::Unsupported)
            }
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |service, cx| {
                service.apply(Observation { started, result }, cx);
            });
        })
        .detach();
        self.refresh_capacity(cx);
        cx.notify();
    }

    fn sample_capacity(&mut self, cx: &mut Context<Self>) {
        for request in self.usage.requests(&self.snapshot, Instant::now()) {
            let worker_request = request.clone();
            let work = cx
                .background_executor()
                .spawn(async move { worker_request.read() });
            cx.spawn(async move |this, cx| {
                let result = work.await;
                let _ = this.update(cx, |service, cx| {
                    if service.usage.finish(request, result, Instant::now()) {
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

    pub fn removal_in_progress(&self, path: &std::path::Path) -> bool {
        self.busy
            .values()
            .flatten()
            .any(|root| path.starts_with(root))
    }

    fn apply(&mut self, observation: Observation, cx: &mut Context<Self>) {
        if observation.started < self.last_observation {
            return;
        }
        self.last_observation = observation.started;
        let previous_snapshot = self.snapshot.clone();
        let previous_error = self.error.clone();
        let was_discovering = self.discovering;
        self.discovering = false;
        match observation.result {
            Ok(snapshot) => {
                self.snapshot = snapshot;
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error.to_string());
                self.snapshot = Snapshot::default();
            }
        }
        if was_discovering || previous_snapshot != self.snapshot || previous_error != self.error {
            cx.notify();
        }
        self.sample_capacity(cx);
    }

    /// # Errors
    /// Rejects disconnected devices and overlapping operations. Backend errors
    /// preserve their system explanation, including busy/authorization failures.
    pub fn operate(
        &mut self,
        action: DeviceAction,
        cx: &mut Context<Self>,
    ) -> Result<Task<Result<OperationOutcome, Error>>, Error> {
        if self.snapshot.volume(action.volume()).is_none() {
            return Err(Error::Disconnected);
        }
        // Lock all volumes of a drive even for a single-volume operation, so a
        // concurrent whole-drive removal cannot race mounting another partition.
        let scope_action = DeviceAction::PowerOff(action.volume().clone());
        let mut scope: Vec<_> = self
            .snapshot
            .removal_volumes(&scope_action)
            .iter()
            .map(|volume| volume.id.clone())
            .collect();
        if scope.is_empty() {
            scope.push(action.volume().clone());
        }
        if scope.iter().any(|id| self.is_busy(id)) {
            return Err(Error::Busy);
        }
        for id in &scope {
            let roots = self
                .snapshot
                .volume(id)
                .map(|volume| volume.mount_points.clone())
                .unwrap_or_default();
            self.busy.insert(id.clone(), roots);
        }
        cx.notify();
        let expected = self.snapshot.clone();
        let work = cx.background_executor().spawn(async move {
            #[cfg(target_os = "linux")]
            {
                let result = super::linux::operate(expected, action).await;
                let started = Instant::now();
                let snapshot = super::linux::refresh().await;
                (result, started, snapshot)
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (expected, action);
                (
                    Err(Error::Unsupported),
                    Instant::now(),
                    Err(Error::Unsupported),
                )
            }
        });
        Ok(cx.spawn(async move |this, cx| {
            let (result, started, snapshot) = work.await;
            let observation = Observation {
                started,
                result: snapshot.clone(),
            };
            let _ = this.update(cx, |service, cx| {
                for id in scope {
                    service.busy.remove(&id);
                }
                service.apply(observation, cx);
                cx.notify();
            });
            Ok(OperationOutcome {
                mount_point: result?,
                snapshot: snapshot?,
            })
        }))
    }
}
