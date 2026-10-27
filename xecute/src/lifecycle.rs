//! Async singleton startup and explicit, dependency-ordered resource cleanup.

use super::*;
use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
};

type ValueFuture = Pin<Box<dyn Future<Output = Result<Arc<DynValue>, InjectError>> + Send>>;
type HookFuture = Pin<Box<dyn Future<Output = Result<(), InjectError>> + Send>>;
pub(super) type AsyncFactory = dyn Fn(Container) -> ValueFuture + Send + Sync;
pub(super) type Hook = dyn Fn(Arc<DynValue>) -> HookFuture + Send + Sync;

#[derive(Default)]
pub(super) struct Lifecycle {
    pub(super) ready: AtomicBool,
    state: tokio::sync::Mutex<State>,
}

#[derive(Default)]
struct State {
    phase: Phase,
    // Retain resources until explicit shutdown, even if startup hooks fail.
    started: Vec<(ProviderKey, Arc<DynValue>)>,
}

#[derive(Default)]
enum Phase {
    #[default]
    Fresh,
    Starting,
    Ready,
    Failed(InjectError),
    Stopping,
    Stopped,
}

impl ProviderDefinition {
    /// Register an asynchronous singleton factory. Declare every dependency it resolves.
    /// The owned container can be used across await points; resolve dependencies with
    /// `container.resolve::<T>()?` after their factories have completed.
    #[must_use]
    pub fn singleton_async<T, F, Fut>(dependencies: Vec<ProviderKey>, factory: F) -> Self
    where
        T: Send + Sync + 'static,
        F: Fn(Container) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, InjectError>> + Send + 'static,
    {
        let mut definition = Self::singleton::<T, _>(dependencies, |_| {
            Err(InjectError::AsyncInitializationRequired(type_name::<T>()))
        });
        definition.async_factory = Some(Arc::new(move |container| {
            let future = factory(container);
            Box::pin(async move { future.await.map(|value| Arc::new(value) as Arc<DynValue>) })
        }));
        definition
    }

    /// Attach an async startup hook to a singleton registration.
    /// `T` must match the registration's type. Runs after dependencies' startup hooks.
    #[must_use]
    pub fn on_init<T, F, Fut>(mut self, hook: F) -> Self
    where
        T: Send + Sync + 'static,
        F: Fn(Arc<T>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), InjectError>> + Send + 'static,
    {
        self.init_hook = Some(typed_hook(hook));
        self
    }

    /// Attach async resource cleanup. Runs in reverse dependency order on shutdown
    /// or failed-startup rollback, including when this provider's init hook failed.
    /// `T` must match the registration's type. Shutdown is explicit, never run in Drop.
    #[must_use]
    pub fn on_shutdown<T, F, Fut>(mut self, hook: F) -> Self
    where
        T: Send + Sync + 'static,
        F: Fn(Arc<T>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), InjectError>> + Send + 'static,
    {
        self.shutdown_hook = Some(typed_hook(hook));
        self
    }
}

fn typed_hook<T, F, Fut>(hook: F) -> Arc<Hook>
where
    T: Send + Sync + 'static,
    F: Fn(Arc<T>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), InjectError>> + Send + 'static,
{
    Arc::new(move |value| match value.downcast::<T>() {
        Ok(value) => Box::pin(hook(value)),
        Err(_) => Box::pin(async { Err(InjectError::Downcast(type_name::<T>())) }),
    })
}

impl Container {
    pub(super) fn ensure_async_dependencies(
        &self,
        key: ProviderKey,
        seen: &mut HashSet<ProviderKey>,
    ) -> Result<(), InjectError> {
        if !seen.insert(key) {
            return Ok(());
        }
        let Some(provider) = self.providers.get(&key) else {
            return Ok(());
        };
        if provider.async_factory.is_some() {
            let initialized = self
                .singletons
                .lock()
                .map_err(|_| InjectError::Poisoned)?
                .get(&key)
                .is_some_and(|slot| slot.get().is_some());
            if !initialized {
                return Err(InjectError::AsyncInitializationRequired(key.name));
            }
        }
        for dependency in &provider.dependencies {
            self.ensure_async_dependencies(*dependency, seen)?;
        }
        Ok(())
    }

    /// Initialize singleton factories and hooks in dependency order before serving.
    /// Clones serialize startup/shutdown; successful startup and failures are cached.
    /// Startup failure rolls back constructed resources and returns the original error.
    /// If cancelled, call shutdown_async to clean up; initialization cannot be retried.
    /// Hooks must not recursively call initialize_async/shutdown_async on this container.
    pub async fn initialize_async(&self) -> Result<(), InjectError> {
        let mut state = self.lifecycle.state.lock().await;
        match &state.phase {
            Phase::Ready => return Ok(()),
            Phase::Failed(error) => return Err(error.clone()),
            Phase::Fresh => {}
            _ => return Err(InjectError::LifecycleStopped),
        }
        state.phase = Phase::Starting;
        let root = Self {
            providers: self.providers.clone(),
            singletons: self.singletons.clone(),
            request_values: None,
            lifecycle: self.lifecycle.clone(),
            async_owner: None,
        };
        let result = root.start(&mut state).await;
        match result {
            Ok(()) => {
                state.phase = Phase::Ready;
                self.lifecycle.ready.store(true, Ordering::Release);
                Ok(())
            }
            Err(error) => {
                // Cleanup is best-effort; preserve the startup failure for callers.
                let _ = root.cleanup(&mut state).await;
                state.phase = Phase::Failed(error.clone());
                Err(error)
            }
        }
    }

    async fn start(&self, state: &mut State) -> Result<(), InjectError> {
        for key in self.startup_order() {
            let definition = &self.providers[&key];
            if definition.scope != Scope::Singleton {
                continue;
            }
            let value = if let Some(factory) = &definition.async_factory {
                let context = Self {
                    async_owner: Some(key),
                    ..self.clone()
                };
                let value = factory(context).await?;
                let slot = self
                    .singletons
                    .lock()
                    .map_err(|_| InjectError::Poisoned)?
                    .entry(key)
                    .or_default()
                    .clone();
                slot.set(Ok(value.clone())).map_err(|_| {
                    InjectError::ConstructionFailed("async singleton already initialized")
                })?;
                value
            } else {
                self.resolve_key(key)?
            };
            state.started.push((key, value.clone()));
            if let Some(hook) = &definition.init_hook {
                hook(value).await?;
            }
        }
        Ok(())
    }

    fn startup_order(&self) -> Vec<ProviderKey> {
        fn visit(
            container: &Container,
            key: ProviderKey,
            seen: &mut HashSet<ProviderKey>,
            order: &mut Vec<ProviderKey>,
        ) {
            if !seen.insert(key) {
                return;
            }
            if let Some(provider) = container.providers.get(&key) {
                for dependency in &provider.dependencies {
                    visit(container, *dependency, seen, order);
                }
                order.push(key);
            }
        }
        let mut keys: Vec<_> = self.providers.keys().copied().collect();
        keys.sort_by_key(|key| key.name);
        let mut seen = HashSet::new();
        let mut order = Vec::new();
        for key in keys {
            visit(self, key, &mut seen, &mut order);
        }
        order
    }

    /// Run singleton cleanup hooks once in reverse startup order, after requests drain.
    /// Continue cleaning up after errors and return the first error. Repeated shutdown
    /// is a no-op. Cancellation leaves remaining hooks for the next shutdown call;
    /// the interrupted hook is not retried. Retained provider Arcs remain accessible.
    pub async fn shutdown_async(&self) -> Result<(), InjectError> {
        let mut state = self.lifecycle.state.lock().await;
        if matches!(state.phase, Phase::Stopped) {
            return Ok(());
        }
        state.phase = Phase::Stopping;
        self.lifecycle.ready.store(false, Ordering::Release);
        let result = self.cleanup(&mut state).await;
        state.phase = Phase::Stopped;
        result
    }

    async fn cleanup(&self, state: &mut State) -> Result<(), InjectError> {
        let mut first_error = None;
        while let Some((key, value)) = state.started.pop() {
            if let Some(hook) = &self.providers[&key].shutdown_hook
                && let Err(error) = hook(value).await
            {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}
