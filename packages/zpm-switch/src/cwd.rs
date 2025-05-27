// Yarn supports passing a cwd as first argument. In the case of yarn switch we want to support this,
// but we don't want to *actually* change the cwd - merely obtain the (future) cwd so we can't pull
// the `packageManager` field from the proper location.
//
// To that end we store the "fake cwd" in this global variable.

use std::cell::RefCell;

use zpm_utils::{Path, PathError};

thread_local!(static FAKE_CWD: RefCell<Option<Path>> = RefCell::new(None));

pub fn set_fake_cwd(cwd: Path) {
    FAKE_CWD.with(|f| {
        *f.borrow_mut() = Some(cwd);
    });
}

pub fn get_fake_cwd() -> Option<Path> {
    FAKE_CWD.with(|f| {
        f.borrow().clone()
    })
}

pub fn get_final_cwd() -> Result<Path, PathError> {
    if let Some(cwd) = get_fake_cwd() {
        Ok(cwd)
    } else {
        Path::current_dir()
    }
}
