use bincode::{Decode, Encode};
use serde::{Deserialize, Deserializer, Serialize};
use zpm_macros::parse_enum;
use zpm_utils::FromFileString;

use crate::{error::Error, primitives::{Descriptor, Ident, Locator, Range}};

#[parse_enum(or_else = |s| Err(Error::InvalidResolution(s.to_string())))]
#[derive(Clone, Debug, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
#[derive_variants(Clone, Debug, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub enum ResolutionSelector {
    #[pattern(spec = r"^(?<descriptor>.*)$")]
    Descriptor {
        descriptor: Descriptor,
    },

    #[pattern(spec = r"^(?<ident>.*)$")]
    Ident {
        ident: Ident,
    },

    #[pattern(spec = r"^(?<parent_descriptor>(?:@[^/*]*/)?[^/*]+)/(?<ident>[^*]+)$")]
    DescriptorIdent {
        parent_descriptor: Descriptor,
        ident: Ident,
    },

    #[pattern(spec = r"^(?<parent_ident>(?:@[^/*]*/)?[^/*]+)/(?<ident>[^*]+)$")]
    IdentIdent {
        parent_ident: Ident,
        ident: Ident,
    },
}

impl ResolutionSelector {
    pub fn target_ident(&self) -> &Ident {
        match self {
            ResolutionSelector::Descriptor(params) => &params.descriptor.ident,
            ResolutionSelector::Ident(params) => &params.ident,
            ResolutionSelector::DescriptorIdent(params) => &params.ident,
            ResolutionSelector::IdentIdent(params) => &params.ident,
        }
    }

    pub fn apply(&self, parent: &Locator, parent_version: &zpm_semver::Version, descriptor: &Descriptor, replacement_range: &Range) -> Option<Range> {
        match self {
            ResolutionSelector::Descriptor(params) => {
                if params.descriptor != *descriptor {
                    return None;
                }

                Some(replacement_range.clone())
            }

            ResolutionSelector::Ident(params) => {
                if params.ident != descriptor.ident {
                    return None;
                }

                Some(replacement_range.clone())
            }

            ResolutionSelector::DescriptorIdent(params) => {
                if params.ident != descriptor.ident {
                    return None;
                }

                if let Range::AnonymousSemver(parent_params) = &params.parent_descriptor.range {
                    if !parent_params.range.check(parent_version) {
                        return None;
                    }
                } else {
                    return None;
                }

                Some(replacement_range.clone())
            }

            ResolutionSelector::IdentIdent(params) => {
                if params.ident != descriptor.ident {
                    return None;
                }

                if params.parent_ident != parent.ident {
                    return None;
                }

                Some(replacement_range.clone())
            }
        }
    }
}

impl<'de> Deserialize<'de> for ResolutionSelector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let s
            = String::deserialize(deserializer)?;

        ResolutionSelector::from_file_string(&s).map_err(serde::de::Error::custom)
    }
}
