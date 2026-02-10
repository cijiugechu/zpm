use std::hash::Hash;

use rkyv::Archive;
use zpm_macro_enum::zpm_enum;
use zpm_utils::{DataType, Hash64, Hash64Builder, Path, ToFileString, UrlEncoded};

use super::{Ident, Locator};

fn format_patch(inner: &UrlEncoded<Locator>, path: &str, checksum: &Option<Hash64>) -> String {
    match checksum {
        Some(checksum) => format!("patch:{}#{}&checksum={}", inner.to_file_string(), path, checksum.to_file_string()),
        None => format!("patch:{}#{}", inner.to_file_string(), path),
    }
}

fn format_registry(ident: &Ident, version: &zpm_semver::Version, url: Option<&String>) -> String {
    match url {
        Some(url) => format!("npm:{}@{}#{}", ident.to_file_string(), version.to_file_string(), url.to_file_string()),
        None => format!("npm:{}@{}", ident.to_file_string(), version.to_file_string()),
    }
}

fn format_workspace_path(path: &Path) -> String {
    if path.is_empty() {
        "workspace:.".to_string()
    } else {
        format!("workspace:{}", path.to_file_string())
    }
}

#[derive(thiserror::Error, Clone, Debug)]
pub enum ReferenceError {
    #[error("Invalid reference: {0}")]
    SyntaxError(String),
}

#[zpm_enum(error = ReferenceError, or_else = |s| Err(ReferenceError::SyntaxError(s.to_string())))]
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord, Hash))]
#[derive_variants(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Archive, rkyv::Serialize, rkyv::Deserialize)]
#[variant_struct_attr(rkyv(derive(PartialEq, Eq, PartialOrd, Ord, Hash)))]
pub enum Reference {
    #[pattern(r"builtin:(?<version>.*)")]
    #[to_file_string(|params| format!("builtin:{}", params.version.to_file_string()))]
    #[to_print_string(|params| DataType::Reference.colorize(&format!("builtin:{}", params.version.to_file_string())))]
    Builtin {
        version: zpm_semver::Version,
    },

    #[pattern(r"npm:(?<version>.*)")]
    #[to_file_string(|params| format!("npm:{}", params.version.to_file_string()))]
    #[to_print_string(|params| DataType::Reference.colorize(&format!("npm:{}", params.version.to_file_string())))]
    Shorthand {
        version: zpm_semver::Version,
    },

    #[pattern(r"npm:(?<ident>(?:@[^#@]+/)?[^#@]+)@(?<version>[^#]*)(?:#(?<url>.*))?")]
    #[to_file_string(|params| format_registry(&params.ident, &params.version, params.url.as_deref()))]
    #[to_print_string(|params| DataType::Reference.colorize(&format_registry(&params.ident, &params.version, params.url.as_deref())))]
    Registry {
        ident: Ident,
        version: zpm_semver::Version,
        url: Option<UrlEncoded<String>>,
    },

    #[pattern(r"file:(?<path>.*\.(?:tgz|tar\.gz))")]
    #[to_file_string(|params| format!("file:{}", params.path))]
    #[to_print_string(|params| DataType::Reference.colorize(&format!("file:{}", params.path)))]
    Tarball {
        path: String,
    },

    #[pattern(r"file:(?<path>.*)")]
    #[to_file_string(|params| format!("file:{}", params.path))]
    #[to_print_string(|params| DataType::Reference.colorize(&format!("file:{}", params.path)))]
    Folder {
        path: String,
    },

    #[pattern(r"link:(?<path>.*)")]
    #[to_file_string(|params| format!("link:{}", params.path))]
    #[to_print_string(|params| DataType::Reference.colorize(&format!("link:{}", params.path)))]
    Link {
        path: String,
    },

    #[pattern(r"portal:(?<path>.*)")]
    #[to_file_string(|params| format!("portal:{}", params.path))]
    #[to_print_string(|params| DataType::Reference.colorize(&format!("portal:{}", params.path)))]
    Portal {
        path: String,
    },

    #[pattern(r"patch:(?<inner>.*)#(?<path>.*)(?:&checksum=(?<checksum>[a-f0-9]*))?$")]
    #[to_file_string(|params| format_patch(&params.inner, &params.path, &params.checksum))]
    #[to_print_string(|params| DataType::Reference.colorize(&format_patch(&params.inner, &params.path, &params.checksum)))]
    #[struct_attr(rkyv(serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator + rkyv::ser::Sharing, <__S as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source)))]
    #[struct_attr(rkyv(deserialize_bounds(__D: rkyv::de::Pooling, <__D as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source)))]
    #[struct_attr(rkyv(bytecheck(bounds(__C: rkyv::validation::ArchiveContext + rkyv::validation::SharedContext, <__C as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source))))]
    Patch {
        #[rkyv(omit_bounds)]
        inner: Box<UrlEncoded<Locator>>,
        path: String,
        checksum: Option<Hash64>,
    },

    #[pattern(r"virtual:(?<hash>[a-f0-9]*)#(?<inner>.*)$")]
    #[to_file_string(|params| format!("virtual:{}#{}", params.hash.to_file_string(), params.inner.to_file_string()))]
    #[to_print_string(|params| format!("{} {}", params.inner.to_print_string(), DataType::Reference.colorize(&format!("[{}]", params.hash.mini()))))]
    #[struct_attr(rkyv(serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator + rkyv::ser::Sharing, <__S as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source)))]
    #[struct_attr(rkyv(deserialize_bounds(__D: rkyv::de::Pooling, <__D as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source)))]
    #[struct_attr(rkyv(bytecheck(bounds(__C: rkyv::validation::ArchiveContext + rkyv::validation::SharedContext, <__C as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source))))]
    Virtual {
        #[rkyv(omit_bounds)]
        inner: Box<Reference>,
        hash: Hash64,
    },

    #[pattern(r"workspace:(?<ident>.*)")]
    #[to_file_string(|params| format!("workspace:{}", params.ident.to_file_string()))]
    #[to_print_string(|params| DataType::Reference.colorize(&format!("workspace:{}", params.ident.to_file_string())))]
    WorkspaceIdent {
        ident: Ident,
    },

    #[pattern(r"workspace:(?<path>.*)")]
    #[to_file_string(|params| format_workspace_path(&params.path))]
    #[to_print_string(|params| DataType::Reference.colorize(&format_workspace_path(&params.path)))]
    WorkspacePath {
        path: Path,
    },

    #[pattern(r"git:(?<git>.*)")]
    #[pattern(r"(?<git>https?://.*\.git#.*)")]
    #[to_file_string(|params| format!("git:{}", params.git.to_file_string()))]
    #[to_print_string(|params| DataType::Reference.colorize(&format!("git:{}", params.git.to_file_string())))]
    Git {
        git: zpm_git::GitReference,
    },

    #[pattern(r"(?<url>https?://.*(?:/.*|\.tgz|\.tar\.gz))")]
    #[to_file_string(|params| params.url.clone())]
    #[to_print_string(|params| DataType::Reference.colorize(&params.url))]
    Url {
        url: String,
    },
}

impl Reference {
    pub fn update_file_string_hash(&self, hasher: &mut Hash64Builder) {
        match self {
            Reference::Builtin(params) => {
                hasher.update(b"builtin:");
                hasher.update(params.version.to_file_string().as_bytes());
            },

            Reference::Shorthand(params) => {
                hasher.update(b"npm:");
                hasher.update(params.version.to_file_string().as_bytes());
            },

            Reference::Registry(params) => {
                hasher.update(b"npm:");
                hasher.update(params.ident.to_file_string().as_bytes());
                hasher.update(b"@");
                hasher.update(params.version.to_file_string().as_bytes());

                if let Some(url) = &params.url {
                    hasher.update(b"#");
                    hasher.update(url.0.as_bytes());
                }
            },

            Reference::Tarball(params) => {
                hasher.update(b"file:");
                hasher.update(params.path.as_bytes());
            },

            Reference::Folder(params) => {
                hasher.update(b"file:");
                hasher.update(params.path.as_bytes());
            },

            Reference::Link(params) => {
                hasher.update(b"link:");
                hasher.update(params.path.as_bytes());
            },

            Reference::Portal(params) => {
                hasher.update(b"portal:");
                hasher.update(params.path.as_bytes());
            },

            Reference::Patch(params) => {
                hasher.update(b"patch:");
                hasher.update(params.inner.to_file_string().as_bytes());
                hasher.update(b"#");
                hasher.update(params.path.as_bytes());

                if let Some(checksum) = &params.checksum {
                    hasher.update(b"&checksum=");
                    hasher.update(checksum.to_file_string().as_bytes());
                }
            },

            Reference::Virtual(params) => {
                hasher.update(b"virtual:");
                hasher.update(params.hash.to_file_string().as_bytes());
                hasher.update(b"#");
                params.inner.update_file_string_hash(hasher);
            },

            Reference::WorkspaceIdent(params) => {
                hasher.update(b"workspace:");
                hasher.update(params.ident.to_file_string().as_bytes());
            },

            Reference::WorkspacePath(params) => {
                hasher.update(b"workspace:");
                if params.path.is_empty() {
                    hasher.update(b".");
                } else {
                    hasher.update(params.path.to_file_string().as_bytes());
                }
            },

            Reference::Git(params) => {
                hasher.update(b"git:");
                hasher.update(params.git.to_file_string().as_bytes());
            },

            Reference::Url(params) => {
                hasher.update(params.url.as_bytes());
            },
        }
    }

    pub fn write_slug_to(&self, output: &mut String) {
        match self {
            Reference::Builtin(params) => {
                output.push_str("builtin-");
                output.push_str(&params.version.to_file_string());
            },

            Reference::Shorthand(params) => {
                output.push_str("npm-");
                output.push_str(&params.version.to_file_string());
            },

            Reference::Git(_) => {
                output.push_str("git");
            },

            Reference::Registry(params) => {
                output.push_str("npm-");
                output.push_str(&params.version.to_file_string());
            },

            Reference::Tarball(_) => {
                output.push_str("file");
            },

            Reference::Folder(_) => {
                output.push_str("file");
            },

            Reference::Link(_) => {
                output.push_str("link");
            },

            Reference::Patch(_) => {
                output.push_str("patch");
            },

            Reference::Portal(_) => {
                output.push_str("portal");
            },

            Reference::Url(_) => {
                output.push_str("url");
            },

            Reference::Virtual(_) => {
                output.push_str("virtual");
            },

            Reference::WorkspaceIdent(_) => {
                output.push_str("workspace");
            },

            Reference::WorkspacePath(_) => {
                output.push_str("workspace");
            },
        }
    }

    pub fn must_bind(&self) -> bool {
        // Keep this implementation in sync w/ Range::must_bind

        if let Reference::Patch(params) = self {
            return params.inner.0.reference.must_bind() || (params.path.as_str() != "<builtin>" && !params.path.as_str().starts_with("~/"));
        }

        matches!(&self, Reference::Link(_) | Reference::Portal(_) | Reference::Tarball(_) | Reference::Folder(_))
    }

    pub fn is_workspace_reference(&self) -> bool {
        matches!(&self, Reference::WorkspaceIdent(_) | Reference::WorkspacePath(_))
    }

    pub fn is_disk_reference(&self) -> bool {
        matches!(&self, Reference::WorkspaceIdent(_) | Reference::WorkspacePath(_) | Reference::Portal(_) | Reference::Link(_))
    }

    pub fn is_virtual_reference(&self) -> bool {
        matches!(&self, Reference::Virtual(_))
    }

    pub fn inner_locator(&self) -> Option<&Locator> {
        // Keep this implementation in sync w/ Range::inner_descriptor

        match self {
            Reference::Patch(params) => {
                Some(&params.inner.0)
            },

            _ => {
                None
            },
        }
    }

    pub fn physical_reference(&self) -> &Reference {
        if let Reference::Virtual(params) = self {
            params.inner.physical_reference()
        } else {
            self
        }
    }

    pub fn slug(&self) -> String {
        let mut slug = String::with_capacity(16);
        self.write_slug_to(&mut slug);
        slug
    }
}
