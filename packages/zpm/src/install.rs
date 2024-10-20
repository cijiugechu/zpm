use std::{collections::{HashMap, HashSet}, hash::Hash, marker::PhantomData};

use arca::Path;
use serde::{Deserialize, Serialize};

use crate::{build, cache::CompositeCache, error::Error, fetcher::{fetch, PackageData}, graph::{GraphCache, GraphIn, GraphOut, GraphTasks}, linker, lockfile::{Lockfile, LockfileEntry}, primitives::{Descriptor, Locator}, print_time, project::Project, resolver::{resolve, Resolution}, system, tree_resolver::{ResolutionTree, TreeResolver}};


#[derive(Clone, Default)]
pub struct InstallContext<'a> {
    pub package_cache: Option<&'a CompositeCache>,
    pub project: Option<&'a Project>,
}

impl<'a> InstallContext<'a> {
    pub fn with_package_cache(mut self, package_cache: Option<&'a CompositeCache>) -> Self {
        self.package_cache = package_cache;
        self
    }

    pub fn with_project(mut self, project: Option<&'a Project>) -> Self {
        self.project = project;
        self
    }
}

#[derive(Clone, Debug)]
pub struct ResolutionResult {
    pub resolution: Resolution,
    pub package_data: Option<PackageData>,
}

impl ResolutionResult {
    pub fn new(resolution: Resolution) -> Self {
        Self {
            resolution,
            package_data: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FetchResult {
    pub resolution: Option<Resolution>,
    pub package_data: PackageData,
}

impl FetchResult {
    pub fn new(package_data: PackageData) -> Self {
        Self {
            resolution: None,
            package_data,
        }
    }

    pub fn into_resolution_result(self) -> ResolutionResult {
        let resolution = self.resolution
            .expect("Expected this fetch result to contain a resolution record to be convertible into a resolution result");

        ResolutionResult {
            resolution,
            package_data: Some(self.package_data),
        }
    }
}

#[derive(Clone, Debug)]
pub enum InstallOpResult {
    Resolved(ResolutionResult),
    Fetched(FetchResult),
}

impl InstallOpResult {
    pub fn expect_resolved(&self) -> &ResolutionResult {
        match self {
            InstallOpResult::Resolved(resolution) => resolution,
            _ => panic!("Expected a resolved result"),
        }
    }

    pub fn expect_fetched(&self) -> &FetchResult {
        match self {
            InstallOpResult::Fetched(fetch) => fetch,
            _ => panic!("Expected a fetched result"),
        }
    }
}

impl<'a> GraphOut<InstallOp<'a>> for InstallOpResult {
    fn graph_follow_ups(&self) -> Vec<InstallOp<'a>> {
        match self {
            InstallOpResult::Resolved(ResolutionResult {resolution, ..}) => {
                let mut follow_ups = vec![InstallOp::Fetch {
                    locator: resolution.locator.clone(),
                }];

                let transitive_dependencies = resolution.dependencies
                    .values()
                    .cloned()
                    .map(|dependency| InstallOp::Resolve {descriptor: dependency});

                follow_ups.extend(transitive_dependencies);
                follow_ups
            },

            InstallOpResult::Fetched(FetchResult {resolution, ..}) => {
                resolution.as_ref().map(|resolution| {
                    let transitive_dependencies = resolution.dependencies
                        .values()
                        .cloned()
                        .map(|dependency| InstallOp::Resolve {descriptor: dependency})
                        .collect();

                    transitive_dependencies
                }).unwrap_or_default()
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum InstallOp<'a> {
    #[allow(dead_code)]
    Phantom(PhantomData<&'a ()>),

    Resolve {
        descriptor: Descriptor,
    },

    Fetch {
        locator: Locator,
    },
}

impl<'a> GraphIn<'a, InstallContext<'a>, InstallOpResult, Error> for InstallOp<'a> {
    fn graph_dependencies(&self) -> Vec<Self> {
        let mut dependencies = vec![];

        match self {
            InstallOp::Phantom(_) =>
                unreachable!("PhantomData should never be instantiated"),

            InstallOp::Resolve {descriptor} => {
                if let Some(parent) = &descriptor.parent {
                    dependencies.push(InstallOp::Fetch {locator: parent.clone()});
                }
            },

            InstallOp::Fetch {locator} => {
                if let Some(parent) = &locator.parent {
                    dependencies.push(InstallOp::Fetch {locator: parent.as_ref().clone()});
                }
            },
        }

        dependencies
    }

    async fn graph_run(self, context: InstallContext<'a>, dependencies: Vec<InstallOpResult>) -> Result<InstallOpResult, Error> {
        match self {
            InstallOp::Phantom(_) =>
                unreachable!("PhantomData should never be instantiated"),

            InstallOp::Resolve {descriptor} => {
                Ok(InstallOpResult::Resolved(resolve(context, descriptor.clone(), dependencies).await?))
            },

            InstallOp::Fetch {locator} => {
                Ok(InstallOpResult::Fetched(fetch(context, &locator.clone(), false, dependencies).await?))
            },

        }
    }
}

struct InstallCache {
    pub lockfile: Lockfile,
}

impl InstallCache {
    pub fn new(lockfile: Lockfile) -> Self {
        Self {
            lockfile,
        }
    }
}

impl<'a> GraphCache<InstallOp<'a>, InstallOpResult> for InstallCache {
    fn graph_cache(&self, op: &InstallOp) -> Option<InstallOpResult> {
        match op {
            InstallOp::Resolve {descriptor} => {
                if let Some(locator) = self.lockfile.resolutions.get(&descriptor) {
                    let entry = self.lockfile.entries.get(locator)
                        .unwrap_or_else(|| panic!("Expected a matching resolution to be found in the lockfile for any resolved locator; not found for {}.", locator));

                    return Some(InstallOpResult::Resolved(ResolutionResult {
                        resolution: entry.resolution.clone(),
                        package_data: None,
                    }));
                }
            },

            _ => {
            },
        }

        None
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallState {
    pub lockfile: Lockfile,
    pub resolution_tree: ResolutionTree,
    pub packages_by_location: HashMap<Path, Locator>,
    pub locations_by_package: HashMap<Locator, Path>,
    pub optional_packages: HashSet<Locator>,
    pub disabled_locators: HashSet<Locator>,
    pub conditional_locators: HashSet<Locator>,
}

#[derive(Clone, Default)]
pub struct Install {
    pub package_data: HashMap<Locator, PackageData>,
    pub install_state: InstallState,
}

impl Install {
    pub async fn finalize(mut self, project: &mut Project) -> Result<(), Error> {
        print_time!("Before link");

        let build = linker::link_project(project, &mut self)
            .await?;

        print_time!("Before build");

        project
            .attach_install_state(self.install_state)?;

        let result = build::BuildManager::new(build)
            .run(project).await?;

        print_time!("Done");

        if !result.build_errors.is_empty() {
            println!("Build errors: {:?}", result.build_errors);
            return Err(Error::Unsupported);
        }

        Ok(())
    }
}

pub struct InstallManager<'a> {
    description: system::Description,
    initial_lockfile: Lockfile,
    roots: Vec<Descriptor>,
    context: InstallContext<'a>,
    result: Install,
}

impl<'a> InstallManager<'a> {
    pub fn new() -> Self {
        InstallManager {
            description: system::Description::from_current(),
            initial_lockfile: Lockfile::new(),
            roots: vec![],
            context: InstallContext::default(),
            result: Install::default(),
        }
    }

    pub fn with_context(mut self, context: InstallContext<'a>) -> Self {
        self.context = context;
        self
    }

    pub fn with_lockfile(mut self, lockfile: Lockfile) -> Self {
        self.initial_lockfile = lockfile;
        self
    }

    pub fn with_roots(mut self, roots: Vec<Descriptor>) -> Self {
        self.roots = roots;
        self
    }

    pub fn with_roots_iter<T: Iterator<Item = Descriptor>>(self, it: T) -> Self {
        self.with_roots(it.collect())
    }

    // fn schedule(&mut self, descriptor: Descriptor) {
    //     if !self.seen.insert(descriptor.clone()) {
    //         return;
    //     }

    //     if descriptor.parent.is_none() {
    //         if let Some(locator) = self.initial_lockfile.resolutions.remove(&descriptor) {
    //             let entry = self.initial_lockfile.entries.get(&locator)
    //                 .unwrap_or_else(|| panic!("Expected a matching resolution to be found in the lockfile for any resolved locator; not found for {}.", locator));

    //             self.record_resolution(descriptor, entry.resolution.clone(), None);
    //             return;
    //         }
    //     }

    //     if let Some(parent) = &descriptor.parent {
    //         if let Some(package_data) = self.result.package_data.get(parent) {
    //             self.ops.push(InstallOp::Resolve {descriptor, parent_data: Some(package_data.clone())});
    //         } else {
    //             self.deferred.entry(parent.clone()).or_default().push(descriptor);
    //         }
    //     } else {
    //         self.ops.push(InstallOp::Resolve {descriptor, parent_data: None});
    //     }
    // }

    // fn record_resolution(&mut self, descriptor: Descriptor, mut resolution: Resolution, package_data: Option<PackageData>) {
    //     for descriptor in resolution.dependencies.values_mut() {
    //         if descriptor.range.must_bind() {
    //             descriptor.parent = Some(resolution.locator.clone());
    //         }
    //     }

    //     let transitive_dependencies = resolution.dependencies
    //         .values()
    //         .cloned();

    //     for descriptor in transitive_dependencies {
    //         self.schedule(descriptor);
    //     }

    //     let parent_data = match &descriptor.parent {
    //         Some(parent) => Some(self.result.package_data.get(parent).expect("Parent data not found").clone()),
    //         None => None,
    //     };

    //     for name in resolution.peer_dependencies.keys().cloned().collect::<Vec<_>>() {
    //         resolution.peer_dependencies.entry(name.type_ident())
    //             .or_insert(PeerRange::Semver(semver::Range::from_str("*").unwrap()));
    //     }

    //     self.result.install_state.lockfile.resolutions.insert(descriptor, resolution.locator.clone());
    //     self.result.install_state.lockfile.entries.insert(resolution.locator.clone(), LockfileEntry {
    //         checksum: None,
    //         resolution: resolution.clone(),
    //     });

    //     if resolution.requirements.is_conditional() {
    //         self.result.install_state.conditional_locators.insert(resolution.locator.clone());

    //         if !resolution.requirements.validate(&self.description) {
    //             self.result.install_state.disabled_locators.insert(resolution.locator.clone());
    //         }
    //     }

    //     if let Some(package_data) = package_data {
    //         self.record_fetch(resolution.locator.clone(), package_data.clone());
    //     } else {
    //         self.ops.push(InstallOp::Fetch {
    //             locator: resolution.locator.clone(),
    //             is_mock_request: self.result.install_state.disabled_locators.contains(&resolution.locator),
    //             parent_data,
    //         });
    //     }
    // }

    // fn record_fetch(&mut self, locator: Locator, package_data: PackageData) {
    //     self.result.package_data.insert(locator.clone(), package_data.clone());

    //     if let Some(deferred) = self.deferred.remove(&locator) {
    //         for descriptor in deferred {
    //             self.seen.remove(&descriptor);
    //             self.schedule(descriptor);
    //         }
    //     }
    // }

    // fn trigger(&mut self) {
    //     while self.running.len() < 100 {
    //         if let Some(op) = self.ops.pop() {
    //             // self.running.push(Box::pin(op.run(self.context.clone())));
    //         } else {
    //             break;
    //         }
    //     }
    // }

    // pub async fn resolve_and_fetch(mut self) -> Result<Install, Error> {
    //     for descriptor in self.roots.clone() {
    //         self.schedule(descriptor);
    //     }

    //     self.trigger();

    //     while let Some(res) = self.running.next().await {
    //         match res {
    //             InstallOpResult::Resolved {descriptor, resolution, package_data} => {
    //                 self.record_resolution(descriptor, resolution, package_data);
    //             }

    //             InstallOpResult::Fetched {locator, package_data} => {
    //                 self.record_fetch(locator, package_data);
    //             }

    //             InstallOpResult::FetchFailed {locator, error} => {
    //                 println!("{}: {:?}", locator, error);
    //             }

    //             InstallOpResult::ResolutionFailed {descriptor, error} => {
    //                 println!("{}: {:?}", descriptor, error);
    //             }
    //         }

    //         self.trigger();
    //     }

    //     if !self.deferred.is_empty() {
    //         panic!("Some deferred descriptors were not resolved");
    //     }

    //     self.result.install_state.resolution_tree = TreeResolver::default()
    //         .with_lockfile(self.result.install_state.lockfile.clone())
    //         .with_roots(self.roots.clone())
    //         .run();

    //     Ok(self.result)
    // }

    pub async fn resolve_and_fetch(mut self) -> Result<Install, Error> {
        let cache = InstallCache::new(self.initial_lockfile.clone());

        let mut graph
            = GraphTasks::new(self.context.clone(), cache);

        for descriptor in self.roots.clone() {
            graph.register(InstallOp::Resolve {
                descriptor: descriptor,
            });
        }

        for entry in graph.run().await.unwrap() {
            match entry {
                (InstallOp::Resolve {descriptor, ..}, InstallOpResult::Resolved(ResolutionResult {resolution, package_data})) => {
                    self.record_resolution(descriptor, resolution, package_data);
                },

                (InstallOp::Fetch {locator, ..}, InstallOpResult::Fetched(FetchResult {package_data, ..})) => {
                    self.record_fetch(locator, package_data);
                },

                _ => panic!("Unsupported install result"),
            }
        }

        self.result.install_state.resolution_tree = TreeResolver::default()
            .with_lockfile(self.result.install_state.lockfile.clone())
            .with_roots(self.roots.clone())
            .run();

        Ok(self.result)
    }

    fn record_resolution(&mut self, descriptor: Descriptor, resolution: Resolution, package_data: Option<PackageData>) {
        self.result.install_state.lockfile.resolutions.insert(descriptor, resolution.locator.clone());
        self.result.install_state.lockfile.entries.insert(resolution.locator.clone(), LockfileEntry {
            checksum: None,
            resolution: resolution.clone(),
        });

        if resolution.requirements.is_conditional() {
            self.result.install_state.conditional_locators.insert(resolution.locator.clone());

            if !resolution.requirements.validate(&self.description) {
                self.result.install_state.disabled_locators.insert(resolution.locator.clone());
            }
        }

        if let Some(package_data) = package_data {
            self.record_fetch(resolution.locator.clone(), package_data.clone());
        }
    }

    fn record_fetch(&mut self, locator: Locator, package_data: PackageData) {
        self.result.package_data.insert(locator.clone(), package_data.clone());
    }
}
