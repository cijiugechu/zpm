use bincode::{Decode, Encode};
use serde::{Deserialize, Deserializer};
use zpm_macros::parse_enum;
use zpm_utils::{impl_serialization_traits, FromFileString, ToFileString, ToHumanString};

use crate::{error::Error, primitives::{Descriptor, Ident, Locator, Range}};

#[parse_enum(or_else = |s| Err(Error::InvalidResolution(s.to_string())))]
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
#[derive_variants(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
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

impl ToFileString for ResolutionSelector {
    fn to_file_string(&self) -> String {
        match self {
            ResolutionSelector::Descriptor(params) => {
                params.descriptor.to_file_string()
            },

            ResolutionSelector::Ident(params) => {
                params.ident.to_file_string()
            },

            ResolutionSelector::DescriptorIdent(params) => {
                params.parent_descriptor.to_file_string() + "/" + &params.ident.to_file_string()
            },

            ResolutionSelector::IdentIdent(params) => {
                params.parent_ident.to_file_string() + "/" + &params.ident.to_file_string()
            },
        }
    }
}

impl ToHumanString for ResolutionSelector {
    fn to_print_string(&self) -> String {
        self.to_file_string()
    }
}

impl_serialization_traits!(ResolutionSelector);
