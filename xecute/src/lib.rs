//! A small, explicit dependency-injection runtime for Axum applications.
//!
//! `Container` owns application providers while `ModuleGraph` verifies module
//! boundaries before the application starts. The older [`Injector`] API is
//! retained for applications migrating from the first preview release.

use std::{
    any::{Any, TypeId, type_name},
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt,
    sync::{Arc, Mutex},
};

pub use xmacros::*;

mod http;
mod openapi;
pub use http::{ApiOperation, ApiResponse, HttpError, HttpStatus, ValidatedJson, ValidatedQuery};
pub use openapi::{DocumentationError, json_schema, openapi_document};

/// Common controller types and macros.
pub mod prelude {
    pub use crate::{HttpError, HttpStatus, Injectable, controller, routes};
    pub use axum::{
        Json,
        response::{IntoResponse, Response},
    };
}

type DynValue = dyn Any + Send + Sync;
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
    /// A factory returned a value of the wrong concrete type.
    Downcast(&'static str),
    /// A synchronization lock was poisoned by a panic.
    Poisoned,
}
impl fmt::Display for InjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(n) => write!(f, "missing provider: {n}"),
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
            Self::Downcast(n) => write!(f, "provider {n} returned an incompatible value"),
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
    factory: Arc<Factory>,
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
            factory: Arc::new(move |c| factory(c).map(|v| Arc::new(v) as Arc<DynValue>)),
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
    /// Construct a provider using dependencies from the container.
    fn provide(container: &Container) -> Result<Self, InjectError>
    where
        Self: Sized;
    /// Build registration metadata for this provider.
    fn definition() -> ProviderDefinition
    where
        Self: Sized,
    {
        ProviderDefinition::new(Self::scope(), Self::dependencies(), Self::provide)
    }
}

/// Thread-safe provider container. Singleton values are cached for its lifetime.
#[derive(Default)]
pub struct Container {
    providers: HashMap<ProviderKey, ProviderDefinition>,
    singletons: Mutex<HashMap<ProviderKey, Arc<DynValue>>>,
}
impl Container {
    #[must_use]
    /// Start constructing and validating a dependency container.
    pub fn builder() -> ContainerBuilder {
        ContainerBuilder::default()
    }
    /// Resolve a dependency, applying its configured lifecycle.
    pub fn resolve<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, InjectError> {
        self.resolve_key(ProviderKey::of::<T>())?
            .downcast::<T>()
            .map_err(|_| InjectError::Downcast(type_name::<T>()))
    }
    /// Resolve a dependency if it is registered; propagate construction failures.
    pub fn resolve_optional<T: Send + Sync + 'static>(
        &self,
    ) -> Result<Option<Arc<T>>, InjectError> {
        match self.resolve::<T>() {
            Ok(v) => Ok(Some(v)),
            Err(InjectError::Missing(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }
    fn resolve_key(&self, key: ProviderKey) -> Result<Arc<DynValue>, InjectError> {
        let definition = self
            .providers
            .get(&key)
            .ok_or(InjectError::Missing(key.name))?;
        if definition.scope == Scope::Request {
            return Err(InjectError::Missing(key.name));
        }
        if definition.scope == Scope::Singleton {
            if let Some(value) = self
                .singletons
                .lock()
                .map_err(|_| InjectError::Poisoned)?
                .get(&key)
                .cloned()
            {
                return Ok(value);
            }
        }
        let value = (definition.factory)(self)?;
        if definition.scope == Scope::Singleton {
            self.singletons
                .lock()
                .map_err(|_| InjectError::Poisoned)?
                .insert(key, value.clone());
        }
        Ok(value)
    }
    #[must_use]
    /// Start a request-local cache for request-scoped dependencies.
    pub fn request(&self) -> RequestContainer<'_> {
        RequestContainer {
            container: self,
            values: HashMap::new(),
        }
    }
}

/// Builds a validated [`Container`].
#[derive(Default)]
pub struct ContainerBuilder {
    providers: Vec<ProviderDefinition>,
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
    /// Validate registrations and dependency relationships before constructing the container.
    pub fn build(self) -> Result<Container, InjectError> {
        let mut providers = HashMap::new();
        for provider in self.providers {
            let key = provider.key;
            if providers.insert(key, provider).is_some() {
                return Err(InjectError::DuplicateProvider(key.name));
            }
        }
        validate_provider_scopes(&providers)?;
        validate_provider_cycles(&providers)?;
        Ok(Container {
            providers,
            singletons: Mutex::new(HashMap::new()),
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
    for provider in providers.values() {
        for dependency in &provider.dependencies {
            if let Some(dependency) = providers.get(dependency) {
                if provider.scope == Scope::Singleton && dependency.scope == Scope::Request {
                    return Err(InjectError::InvalidScopeDependency {
                        owner: provider.key.name,
                        dependency: dependency.key.name,
                    });
                }
            }
        }
    }
    Ok(())
}

/// Per-request provider cache. Request-scoped values cannot escape this type.
pub struct RequestContainer<'a> {
    container: &'a Container,
    values: HashMap<ProviderKey, Arc<DynValue>>,
}
impl RequestContainer<'_> {
    /// Resolve a dependency, applying its configured lifecycle.
    pub fn resolve<T: Send + Sync + 'static>(&mut self) -> Result<Arc<T>, InjectError> {
        let key = ProviderKey::of::<T>();
        let definition = self
            .container
            .providers
            .get(&key)
            .ok_or(InjectError::Missing(key.name))?;
        let value = if definition.scope == Scope::Request {
            if let Some(v) = self.values.get(&key) {
                v.clone()
            } else {
                let v = (definition.factory)(self.container)?;
                self.values.insert(key, v.clone());
                v
            }
        } else {
            self.container.resolve_key(key)?
        };
        value
            .downcast::<T>()
            .map_err(|_| InjectError::Downcast(type_name::<T>()))
    }
}

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
    modules: HashMap<&'static str, ModuleDefinition>,
}
impl ModuleGraph {
    #[must_use]
    /// Create a new instance from the supplied definition inputs.
    pub fn new(modules: Vec<ModuleDefinition>) -> Self {
        Self {
            modules: modules.into_iter().map(|m| (m.name, m)).collect(),
        }
    }
    /// Validate registrations and dependency relationships before constructing the container.
    pub fn build(self) -> Result<Container, InjectError> {
        for module in self.modules.values() {
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
                if !self.modules.contains_key(import) {
                    return Err(InjectError::InvalidImport {
                        module: module.name,
                        import,
                    });
                }
            }
        }
        let mut builder = Container::builder();
        for module in self.modules.into_values() {
            for provider in module.providers {
                builder = builder.provider(provider);
            }
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
