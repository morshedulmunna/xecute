//! Application architecture tools, macros, and a dependency-injection runtime for Axum.
//!
//! `Container` owns application providers while `ModuleGraph` verifies module
//! boundaries before the application starts. The older [`Injector`] API is
//! retained for applications migrating from the first preview release.

use std::{
    any::{Any, TypeId, type_name},
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt,
    sync::{Arc, Mutex, OnceLock},
};

pub use xmacros::*;

/// HTTP middleware and application pipeline composition.
pub mod middleware;
pub use middleware::Application;

/// Executable application, controller, and route guards.
pub mod guards;

/// JWT bearer authentication, verified principals, and role/permission policies.
#[cfg(feature = "auth")]
pub mod auth;

/// Streaming multipart uploads with private local and optional S3 storage.
#[cfg(feature = "upload")]
pub mod upload;

/// In-process HTTP tests and isolated provider overrides.
#[cfg(feature = "testing")]
pub mod testing;

/// Typed command/query dispatch, event fan-out, and shared CQRS pipelines.
#[cfg(feature = "cqrs")]
pub mod cqrs;

/// Bounded process-local background jobs and interval scheduling.
#[cfg(feature = "jobs")]
pub mod jobs;

/// Explicit TTL caching with memory and optional Redis backends.
#[cfg(feature = "cache")]
pub mod cache;

/// Optional SQLx PostgreSQL pools, migrations, and lifecycle integration.
#[cfg(feature = "postgres")]
pub mod database;

/// Typed configuration readers and redacted validation errors.
pub mod config;

mod lifecycle;

mod scoped;
pub use scoped::{Inject, with_container};

/// Safe HTTP error envelopes, normalization, and request correlation.
pub mod exceptions;

mod http;
mod openapi;
/// Explicit route version groups and per-version OpenAPI metadata.
pub mod versioning;
pub use http::{ApiOperation, ApiResponse, HttpError, HttpStatus, ValidatedJson, ValidatedQuery};
pub use openapi::{DocumentationError, json_schema, openapi_document};
pub use versioning::{ApiVersion, ApiVersions, VersionError};

/// Common controller types and macros.
pub mod prelude {
    pub use crate::{
        Application, HttpError, HttpStatus, Inject, Injectable, controller, middleware, routes,
    };
    pub use axum::{
        Json,
        response::{IntoResponse, Response},
    };
}

type DynValue = dyn Any + Send + Sync;
type Cache = Mutex<HashMap<ProviderKey, Arc<OnceLock<Result<Arc<DynValue>, InjectError>>>>>;
type Factory = dyn Fn(&Container) -> Result<Arc<DynValue>, InjectError> + Send + Sync;

/// The lifecycle of a provider instance.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Scope {
    #[default]
    /// Reuse one instance for the container lifetime.
    Singleton,
    /// Construct a new instance for each resolution.
    Transient,
    /// Reuse an instance within one request-local container.
    Request,
}

/// A type-safe key used to identify a provider.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProviderKey {
    type_id: TypeId,
    name: &'static str,
}
impl ProviderKey {
    #[must_use]
    /// Identify a provider by its concrete Rust type.
    pub fn of<T: 'static>() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            name: type_name::<T>(),
        }
    }
    #[must_use]
    /// Return the diagnostic type name.
    pub const fn name(self) -> &'static str {
        self.name
    }
}

/// Errors discovered while constructing or validating an application.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InjectError {
    /// A required provider was not registered.
    Missing(&'static str),
    /// More than one module definition uses the same module name.
    DuplicateModule(&'static str),
    /// More than one registration uses the same provider key.
    DuplicateProvider(&'static str),
    /// An importing module requested a provider that was not exported.
    PrivateProviderAccess {
        /// Provider type involved in the validation failure.
        provider: &'static str,
        /// Module owning the invalid relationship.
        module: &'static str,
    },
    /// A module exported a provider it does not own.
    InvalidExport {
        /// Provider type involved in the validation failure.
        provider: &'static str,
        /// Module owning the invalid relationship.
        module: &'static str,
    },
    /// A referenced module was not present in the module graph.
    InvalidImport {
        /// Module owning the invalid relationship.
        module: &'static str,
        /// Referenced imported module.
        import: &'static str,
    },
    /// Dependency resolution encountered a cycle.
    Circular(Vec<&'static str>),
    /// A provider depends on a shorter-lived provider.
    InvalidScopeDependency {
        /// Provider that owns the dependency relationship.
        owner: &'static str,
        /// Dependency with an incompatible scope.
        dependency: &'static str,
    },
    /// A request-scoped provider was resolved without an active request.
    RequestScopeRequired(&'static str),
    /// A factory resolved a dependency not declared in its metadata.
    UndeclaredDependency {
        /// Factory performing the resolution.
        owner: &'static str,
        /// Dependency missing from the factory metadata.
        dependency: &'static str,
    },
    /// A factory returned a value of the wrong concrete type.
    Downcast(&'static str),
    /// A provider could not be constructed; the static message must not contain secrets.
    ConstructionFailed(&'static str),
    /// A synchronization lock was poisoned by a panic.
    Poisoned,
    /// Async construction or lifecycle hooks require initialize_async before serving.
    AsyncInitializationRequired(&'static str),
    /// Lifecycle hooks are supported only on singleton providers.
    InvalidLifecycleScope(&'static str),
    /// Initialization or shutdown was cancelled, or the container has stopped.
    LifecycleStopped,
}
impl fmt::Display for InjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(n) => write!(f, "missing provider: {n}"),
            Self::DuplicateModule(n) => write!(f, "duplicate module: {n}"),
            Self::DuplicateProvider(n) => write!(f, "duplicate provider: {n}"),
            Self::PrivateProviderAccess { provider, module } => write!(
                f,
                "private provider {provider} is not visible to module {module}"
            ),
            Self::InvalidExport { provider, module } => write!(
                f,
                "module {module} exports unregistered provider {provider}"
            ),
            Self::InvalidImport { module, import } => {
                write!(f, "module {module} imports unknown module {import}")
            }
            Self::Circular(chain) => write!(f, "circular dependency:\n{}", chain.join("\n  -> ")),
            Self::InvalidScopeDependency { owner, dependency } => write!(
                f,
                "invalid scope dependency: singleton {owner} depends on request-scoped {dependency}"
            ),
            Self::RequestScopeRequired(n) => write!(f, "request scope required for provider: {n}"),
            Self::UndeclaredDependency { owner, dependency } => write!(
                f,
                "provider {owner} resolved undeclared dependency {dependency}"
            ),
            Self::Downcast(n) => write!(f, "provider {n} returned an incompatible value"),
            Self::ConstructionFailed(reason) => write!(f, "provider construction failed: {reason}"),
            Self::AsyncInitializationRequired(n) => {
                write!(f, "provider {n} requires initialize_async")
            }
            Self::InvalidLifecycleScope(n) => {
                write!(f, "lifecycle hooks require singleton scope: {n}")
            }
            Self::LifecycleStopped => write!(f, "container lifecycle stopped or interrupted"),
            Self::Poisoned => write!(f, "container lock was poisoned"),
        }
    }
}
impl std::error::Error for InjectError {}

/// A provider registration. Dependencies are metadata used for startup validation.
pub struct ProviderDefinition {
    key: ProviderKey,
    scope: Scope,
    dependencies: Vec<ProviderKey>,
    optional_dependencies: Vec<ProviderKey>,
    factory: Arc<Factory>,
    async_factory: Option<Arc<lifecycle::AsyncFactory>>,
    init_hook: Option<Arc<lifecycle::Hook>>,
    shutdown_hook: Option<Arc<lifecycle::Hook>>,
}
impl fmt::Debug for ProviderDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderDefinition")
            .field("key", &self.key)
            .field("scope", &self.scope)
            .field("dependencies", &self.dependencies)
            .finish()
    }
}
impl ProviderDefinition {
    #[must_use]
    /// Create a new instance from the supplied definition inputs.
    pub fn new<T, F>(scope: Scope, dependencies: Vec<ProviderKey>, factory: F) -> Self
    where
        T: Send + Sync + 'static,
        F: Fn(&Container) -> Result<T, InjectError> + Send + Sync + 'static,
    {
        Self {
            key: ProviderKey::of::<T>(),
            scope,
            dependencies,
            optional_dependencies: Vec::new(),
            factory: Arc::new(move |c| factory(c).map(|v| Arc::new(v) as Arc<DynValue>)),
            async_factory: None,
            init_hook: None,
            shutdown_hook: None,
        }
    }
    #[must_use]
    /// Register a factory whose result is cached for the container lifetime.
    pub fn singleton<T, F>(dependencies: Vec<ProviderKey>, factory: F) -> Self
    where
        T: Send + Sync + 'static,
        F: Fn(&Container) -> Result<T, InjectError> + Send + Sync + 'static,
    {
        Self::new(Scope::Singleton, dependencies, factory)
    }
    #[must_use]
    /// Register a factory reused within each independent request context.
    pub fn request<T, F>(dependencies: Vec<ProviderKey>, factory: F) -> Self
    where
        T: Send + Sync + 'static,
        F: Fn(&Container) -> Result<T, InjectError> + Send + Sync + 'static,
    {
        Self::new(Scope::Request, dependencies, factory)
    }
    #[must_use]
    /// Register a factory constructing a fresh instance on every resolution.
    pub fn transient<T, F>(dependencies: Vec<ProviderKey>, factory: F) -> Self
    where
        T: Send + Sync + 'static,
        F: Fn(&Container) -> Result<T, InjectError> + Send + Sync + 'static,
    {
        Self::new(Scope::Transient, dependencies, factory)
    }
    #[must_use]
    /// Register one preconstructed value, useful for application settings and test doubles.
    pub fn value<T: Send + Sync + 'static>(value: T) -> Self {
        let value: Arc<DynValue> = Arc::new(value);
        Self {
            key: ProviderKey::of::<T>(),
            scope: Scope::Singleton,
            dependencies: Vec::new(),
            optional_dependencies: Vec::new(),
            factory: Arc::new(move |_| Ok(value.clone())),
            async_factory: None,
            init_hook: None,
            shutdown_hook: None,
        }
    }
    #[must_use]
    /// Mark one declared dependency as optional for startup registration validation.
    pub fn optional_dependency<T: 'static>(mut self) -> Self {
        self.optional_dependencies.push(ProviderKey::of::<T>());
        self
    }
    #[must_use]
    /// Return the type key identifying this provider.
    pub fn key(&self) -> ProviderKey {
        self.key
    }
    #[must_use]
    /// Return the lifetime used when resolving this provider.
    pub fn scope(&self) -> Scope {
        self.scope
    }
    #[must_use]
    /// List the provider keys required by this factory.
    pub fn dependencies(&self) -> &[ProviderKey] {
        &self.dependencies
    }
}

/// Types generated by `#[injectable]` can register themselves with a container.
pub trait Provider: Send + Sync + 'static {
    /// Return the lifetime used when resolving this provider.
    fn scope() -> Scope {
        Scope::Singleton
    }
    /// List the provider keys required by this factory.
    fn dependencies() -> Vec<ProviderKey> {
        Vec::new()
    }
    /// Optional dependency keys which may be absent at startup.
    fn optional_dependencies() -> Vec<ProviderKey> {
        Vec::new()
    }
    /// Construct a provider using dependencies from the container.
    fn provide(container: &Container) -> Result<Self, InjectError>
    where
        Self: Sized;
    /// Build registration metadata for this provider.
    fn definition() -> ProviderDefinition
    where
        Self: Sized,
    {
        let mut definition =
            ProviderDefinition::new(Self::scope(), Self::dependencies(), Self::provide);
        definition.optional_dependencies = Self::optional_dependencies();
        definition
    }
}

/// Thread-safe provider container with shared singleton and isolated request caches.
#[derive(Clone, Default)]
pub struct Container {
    providers: Arc<HashMap<ProviderKey, ProviderDefinition>>,
    singletons: Arc<Cache>,
    request_values: Option<Arc<Cache>>,
    lifecycle: Arc<lifecycle::Lifecycle>,
    async_owner: Option<ProviderKey>,
}

thread_local! {
    static RESOLVING: RefCell<Vec<(usize, ProviderKey)>> = const { RefCell::new(Vec::new()) };
}
struct ResolutionGuard;
impl Drop for ResolutionGuard {
    fn drop(&mut self) {
        RESOLVING.with(|path| {
            path.borrow_mut().pop();
        });
    }
}
impl Container {
    #[must_use]
    /// Start constructing and validating a dependency container.
    pub fn builder() -> ContainerBuilder {
        ContainerBuilder::default()
    }
    /// Resolve a provider; request providers require a request context.
    pub fn resolve<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, InjectError> {
        self.resolve_key(ProviderKey::of::<T>())?
            .downcast::<T>()
            .map_err(|_| InjectError::Downcast(type_name::<T>()))
    }
    /// Return None only for an unregistered optional provider; propagate factory failures.
    pub fn resolve_optional<T: Send + Sync + 'static>(
        &self,
    ) -> Result<Option<Arc<T>>, InjectError> {
        if !self.providers.contains_key(&ProviderKey::of::<T>()) {
            return Ok(None);
        }
        self.resolve().map(Some)
    }
    fn resolve_key(&self, key: ProviderKey) -> Result<Arc<DynValue>, InjectError> {
        let definition = self
            .providers
            .get(&key)
            .ok_or(InjectError::Missing(key.name))?;
        // Fail before entering a synchronous OnceLock factory, so an early resolve
        // cannot permanently cache AsyncInitializationRequired in its dependents.
        self.ensure_async_dependencies(key, &mut HashSet::new())?;
        let identity = Arc::as_ptr(&self.providers) as usize;
        RESOLVING.with(|path| {
            let mut path = path.borrow_mut();
            if !path.iter().any(|(id, _)| *id == identity)
                && let Some(owner) = self.async_owner
                && !self.providers[&owner].dependencies.contains(&key)
            {
                return Err(InjectError::UndeclaredDependency {
                    owner: owner.name,
                    dependency: key.name,
                });
            }
            if let Some(index) = path.iter().position(|entry| *entry == (identity, key)) {
                let mut chain = path[index..]
                    .iter()
                    .map(|entry| entry.1.name)
                    .collect::<Vec<_>>();
                chain.push(key.name);
                return Err(InjectError::Circular(chain));
            }
            if let Some((id, parent)) = path.last()
                && *id == identity
                && !self.providers[parent].dependencies.contains(&key)
            {
                return Err(InjectError::UndeclaredDependency {
                    owner: parent.name,
                    dependency: key.name,
                });
            }
            path.push((identity, key));
            Ok(())
        })?;
        let _guard = ResolutionGuard;
        let cache = match definition.scope {
            Scope::Transient => return (definition.factory)(self),
            Scope::Singleton => &self.singletons,
            Scope::Request => self
                .request_values
                .as_ref()
                .ok_or(InjectError::RequestScopeRequired(key.name))?,
        };
        // Lock only to obtain the per-provider slot. Factories can resolve other slots.
        let slot = cache
            .lock()
            .map_err(|_| InjectError::Poisoned)?
            .entry(key)
            .or_default()
            .clone();
        // Successful instances and construction errors are deterministic within the scope.
        // OnceLock serializes concurrent initialization without holding the cache lock.
        if definition.async_factory.is_some() {
            return slot
                .get()
                .cloned()
                .unwrap_or(Err(InjectError::AsyncInitializationRequired(key.name)));
        }
        slot.get_or_init(|| {
            if definition.scope == Scope::Singleton {
                // Singletons must never retain the first HTTP request's resolution context.
                let root = Self {
                    providers: self.providers.clone(),
                    singletons: self.singletons.clone(),
                    request_values: None,
                    lifecycle: self.lifecycle.clone(),
                    async_owner: None,
                };
                (definition.factory)(&root)
            } else {
                (definition.factory)(self)
            }
        })
        .clone()
    }
    #[must_use]
    /// Start an independent request cache while sharing application singletons.
    pub fn request(&self) -> RequestScope {
        RequestScope {
            container: Self {
                providers: self.providers.clone(),
                singletons: self.singletons.clone(),
                request_values: Some(Arc::default()),
                lifecycle: self.lifecycle.clone(),
                async_owner: self.async_owner,
            },
        }
    }
    /// Eagerly construct singletons so factory failures are reported before listening.
    /// Request and transient factories are constructed only when resolved.
    pub fn initialize(&self) -> Result<(), InjectError> {
        if !self
            .lifecycle
            .ready
            .load(std::sync::atomic::Ordering::Acquire)
            && let Some(definition) = self.providers.values().find(|p| {
                p.async_factory.is_some() || p.init_hook.is_some() || p.shutdown_hook.is_some()
            })
        {
            return Err(InjectError::AsyncInitializationRequired(
                definition.key.name,
            ));
        }
        for definition in self
            .providers
            .values()
            .filter(|definition| definition.scope == Scope::Singleton)
        {
            self.resolve_key(definition.key)?;
        }
        Ok(())
    }
}

/// Builds a validated [`Container`].
#[derive(Default)]
pub struct ContainerBuilder {
    providers: Vec<ProviderDefinition>,
    overrides: Vec<ProviderDefinition>,
}
impl ContainerBuilder {
    #[must_use]
    /// Append a provider registration to this definition.
    pub fn provider(mut self, provider: ProviderDefinition) -> Self {
        self.providers.push(provider);
        self
    }
    #[must_use]
    /// Register a type implementing the provider factory contract.
    pub fn injectable<T: Provider>(self) -> Self {
        self.provider(T::definition())
    }
    /// Replace an existing provider for an isolated test container.
    #[must_use]
    pub fn override_provider(mut self, provider: ProviderDefinition) -> Self {
        self.overrides.push(provider);
        self
    }
    /// Register a preconstructed value such as a trait adapter or test double.
    #[must_use]
    pub fn value<T: Send + Sync + 'static>(self, value: T) -> Self {
        self.provider(ProviderDefinition::value(value))
    }
    #[cfg(feature = "testing")]
    pub(crate) fn into_definitions(self) -> Vec<ProviderDefinition> {
        self.providers
    }
    /// Validate registrations and dependency relationships before constructing the container.
    pub fn build(self) -> Result<Container, InjectError> {
        let mut providers = HashMap::new();
        for provider in self.providers {
            let key = provider.key;
            if providers.insert(key, provider).is_some() {
                return Err(InjectError::DuplicateProvider(key.name));
            }
        }
        let mut overrides = HashSet::new();
        for replacement in self.overrides {
            let key = replacement.key;
            if !overrides.insert(key) {
                return Err(InjectError::DuplicateProvider(key.name));
            }
            if !providers.contains_key(&key) {
                return Err(InjectError::Missing(key.name));
            }
            providers.insert(key, replacement);
        }
        for provider in providers.values() {
            for dependency in &provider.dependencies {
                if !providers.contains_key(dependency)
                    && !provider.optional_dependencies.contains(dependency)
                {
                    return Err(InjectError::Missing(dependency.name));
                }
            }
        }
        for provider in providers.values() {
            if provider.scope != Scope::Singleton
                && (provider.init_hook.is_some() || provider.shutdown_hook.is_some())
            {
                return Err(InjectError::InvalidLifecycleScope(provider.key.name));
            }
        }
        validate_provider_scopes(&providers)?;
        validate_provider_cycles(&providers)?;
        Ok(Container {
            providers: Arc::new(providers),
            singletons: Arc::default(),
            request_values: None,
            lifecycle: Arc::default(),
            async_owner: None,
        })
    }
}

fn validate_provider_cycles(
    providers: &HashMap<ProviderKey, ProviderDefinition>,
) -> Result<(), InjectError> {
    fn visit(
        key: ProviderKey,
        providers: &HashMap<ProviderKey, ProviderDefinition>,
        visiting: &mut Vec<ProviderKey>,
        visited: &mut HashSet<ProviderKey>,
    ) -> Result<(), InjectError> {
        if let Some(index) = visiting.iter().position(|entry| *entry == key) {
            let mut chain = visiting[index..]
                .iter()
                .map(|entry| entry.name)
                .collect::<Vec<_>>();
            chain.push(key.name);
            return Err(InjectError::Circular(chain));
        }
        if !visited.insert(key) {
            return Ok(());
        }
        visiting.push(key);
        if let Some(provider) = providers.get(&key) {
            for dependency in &provider.dependencies {
                if providers.contains_key(dependency) {
                    visit(*dependency, providers, visiting, visited)?;
                }
            }
        }
        visiting.pop();
        Ok(())
    }
    let mut visiting = Vec::new();
    let mut visited = HashSet::new();
    for key in providers.keys().copied() {
        visit(key, providers, &mut visiting, &mut visited)?;
    }
    Ok(())
}
fn validate_provider_scopes(
    providers: &HashMap<ProviderKey, ProviderDefinition>,
) -> Result<(), InjectError> {
    for provider in providers
        .values()
        .filter(|provider| provider.scope == Scope::Singleton)
    {
        let mut pending = provider.dependencies.clone();
        let mut visited = HashSet::new();
        while let Some(key) = pending.pop() {
            if !visited.insert(key) {
                continue;
            }
            if let Some(dependency) = providers.get(&key) {
                if dependency.scope == Scope::Request {
                    return Err(InjectError::InvalidScopeDependency {
                        owner: provider.key.name,
                        dependency: key.name,
                    });
                }
                pending.extend_from_slice(&dependency.dependencies);
            }
        }
    }
    Ok(())
}

/// Owned request-local provider cache. Clones share only this request's values.
/// Returned Arcs can outlive the HTTP request if retained by application code.
#[derive(Clone)]
pub struct RequestScope {
    container: Container,
}
impl RequestScope {
    pub(crate) fn belongs_to(&self, container: &Container) -> bool {
        Arc::ptr_eq(&self.container.providers, &container.providers)
    }
    /// Resolve nested request, transient, and singleton dependencies in this request.
    pub fn resolve<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, InjectError> {
        self.container.resolve()
    }
    /// Resolve an optional provider without hiding factory failures.
    pub fn resolve_optional<T: Send + Sync + 'static>(
        &self,
    ) -> Result<Option<Arc<T>>, InjectError> {
        self.container.resolve_optional()
    }
}

/// Compatibility name for the owned request scope; the lifetime no longer restricts ownership.
/// Existing `RequestContainer<'a>` type annotations continue to compile.
pub type RequestContainer<'a> = RequestScope;

/// Declarative module metadata, used to validate imports and exports.
#[derive(Debug)]
pub struct ModuleDefinition {
    /// Unique module type name.
    pub name: &'static str,
    /// Provider registrations owned by this module.
    pub providers: Vec<ProviderDefinition>,
    /// Module names whose exports this module can access.
    pub imports: Vec<&'static str>,
    /// Provider keys available to importing modules.
    pub exports: Vec<ProviderKey>,
    /// Whether exports are visible throughout the module graph.
    pub global: bool,
}
impl ModuleDefinition {
    #[must_use]
    /// Create a new instance from the supplied definition inputs.
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            providers: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            global: false,
        }
    }
    #[must_use]
    /// Append a provider registration to this definition.
    pub fn provider(mut self, provider: ProviderDefinition) -> Self {
        self.providers.push(provider);
        self
    }
    #[must_use]
    /// Import providers exported by the named module.
    pub fn import(mut self, module: &'static str) -> Self {
        self.imports.push(module);
        self
    }
    #[must_use]
    /// Make the specified provider visible to importing modules.
    pub fn export<T: 'static>(mut self) -> Self {
        self.exports.push(ProviderKey::of::<T>());
        self
    }
    #[must_use]
    /// Make this module’s exports visible to all modules.
    pub fn global(mut self) -> Self {
        self.global = true;
        self
    }
}

/// Validates module metadata and flattens modules into a container.
pub struct ModuleGraph {
    modules: Vec<ModuleDefinition>,
}
impl ModuleGraph {
    #[must_use]
    /// Create a new instance from the supplied definition inputs.
    pub fn new(modules: Vec<ModuleDefinition>) -> Self {
        Self { modules }
    }
    /// Validate registrations and dependency relationships before constructing the container.
    pub fn build(self) -> Result<Container, InjectError> {
        self.build_with_overrides(Vec::new())
    }
    /// Validate module metadata and replace selected provider factories for tests.
    pub fn build_with_overrides(
        self,
        overrides: Vec<ProviderDefinition>,
    ) -> Result<Container, InjectError> {
        let mut modules = HashMap::new();
        for module in self.modules {
            let name = module.name;
            if modules.insert(name, module).is_some() {
                return Err(InjectError::DuplicateModule(name));
            }
        }
        for module in modules.values() {
            let owned: HashSet<_> = module
                .providers
                .iter()
                .map(ProviderDefinition::key)
                .collect();
            for export in &module.exports {
                if !owned.contains(export) {
                    return Err(InjectError::InvalidExport {
                        provider: export.name,
                        module: module.name,
                    });
                }
            }
            for import in &module.imports {
                if !modules.contains_key(import) {
                    return Err(InjectError::InvalidImport {
                        module: module.name,
                        import,
                    });
                }
            }
        }
        for module in modules.values() {
            let mut visible: HashSet<_> = module
                .providers
                .iter()
                .map(ProviderDefinition::key)
                .collect();
            for imported in modules
                .values()
                .filter(|candidate| candidate.global || module.imports.contains(&candidate.name))
            {
                visible.extend(imported.exports.iter().copied());
            }
            for provider in &module.providers {
                for dependency in &provider.dependencies {
                    let registered = modules.values().any(|candidate| {
                        candidate
                            .providers
                            .iter()
                            .any(|provider| provider.key == *dependency)
                    });
                    if registered && !visible.contains(dependency) {
                        return Err(InjectError::PrivateProviderAccess {
                            provider: dependency.name,
                            module: module.name,
                        });
                    }
                }
            }
        }
        let mut builder = Container::builder();
        for module in modules.into_values() {
            for provider in module.providers {
                builder = builder.provider(provider);
            }
        }
        for replacement in overrides {
            builder = builder.override_provider(replacement);
        }
        builder.build()
    }
}

// Compatibility API for the initial local migration.
/// Compatibility construction contract for applications using the original injector.
pub trait Injectable: Sized {
    /// Construct a value through the compatibility injector.
    fn inject(injector: &Injector) -> Result<Self, InjectError>;
}
impl<T: Injectable + 'static> Injectable for Arc<T> {
    /// Construct a value through the compatibility injector.
    fn inject(injector: &Injector) -> Result<Self, InjectError> {
        injector.resolve::<T>().map(Arc::new)
    }
}
/// Compatibility injector storing cloneable application values.
pub struct Injector {
    values: HashMap<TypeId, Box<dyn Any>>,
}
thread_local! { static PATH: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) }; }
impl Injector {
    #[must_use]
    /// Create a new instance from the supplied definition inputs.
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }
    #[must_use]
    /// Register a cloneable value for compatibility injection.
    pub fn value<T: Clone + 'static>(mut self, value: T) -> Self {
        self.values.insert(TypeId::of::<T>(), Box::new(value));
        self
    }
    /// Resolve a dependency, applying its configured lifecycle.
    pub fn resolve<T: Injectable + 'static>(&self) -> Result<T, InjectError> {
        let name = type_name::<T>();
        PATH.with(|path| {
            let mut path = path.borrow_mut();
            if let Some(index) = path.iter().position(|entry| *entry == name) {
                let mut chain = path[index..].to_vec();
                chain.push(name);
                return Err(InjectError::Circular(chain));
            }
            path.push(name);
            drop(path);
            let result = T::inject(self);
            PATH.with(|path| {
                path.borrow_mut().pop();
            });
            result
        })
    }
    /// Read a registered cloneable value or return a missing-provider error.
    pub fn value_of<T: Clone + 'static>(&self) -> Result<T, InjectError> {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>())
            .cloned()
            .ok_or(InjectError::Missing(type_name::<T>()))
    }
}
impl Default for Injector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct A;
    impl Provider for A {
        /// Construct a provider using dependencies from the container.
        fn provide(_: &Container) -> Result<Self, InjectError> {
            Ok(Self)
        }
    }
    #[test]
    fn singleton_is_cached() {
        let c = Container::builder().injectable::<A>().build().unwrap();
        assert!(Arc::ptr_eq(
            &c.resolve::<A>().unwrap(),
            &c.resolve::<A>().unwrap()
        ));
    }
}
