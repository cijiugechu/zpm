use crate::{error::Error, install::{FetchResult, InstallOpResult}};

use super::PackageData;

pub fn fetch_locator(path: &str, dependencies: Vec<InstallOpResult>) -> Result<FetchResult, Error> {
    let parent_data = dependencies[0].as_fetched();

    let package_directory = parent_data.package_data
        .context_directory()
        .with_join_str(path);

    Ok(FetchResult {
        resolution: None,
        package_data: PackageData::Local {
            package_directory,
            discard_from_lookup: false,
        },
    })
}
