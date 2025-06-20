use std::process::ExitCode;

use clipanion::{prelude::*, program, Environment};
use zpm_macros::track_time;
use zpm_switch::{extract_bin_meta, BinMeta};
use zpm_utils::Path;

mod debug;

mod add;
mod bin;
mod config;
mod config_get;
mod config_set;
mod constraints;
mod dlx;
mod exec;
mod init;
mod install;
mod node;
mod pack;
mod remove;
mod run;
mod set_version;
mod up;
mod workspaces_list;
mod workspace;

program!(YarnCli, [
    debug::check_descriptor::CheckDescriptor,
    debug::check_ident::CheckIdent,
    debug::check_range::CheckRange,
    debug::check_reference::CheckReference,
    debug::check_semver_version::CheckSemverVersion,
    debug::print_platform::PrintPlatform,

    add::Add,
    bin::BinList,
    bin::Bin,
    config::Config,
    config_get::ConfigGet,
    config_set::ConfigSet,
    constraints::Constraints,
    dlx::DlxWithPackages,
    dlx::Dlx,
    exec::Exec,
    init::Init,
    install::Install,
    set_version::SetVersion,
    node::Node,
    pack::Pack,
    remove::Remove,
    run::Run,
    up::Up,
    workspaces_list::WorkspacesList,
    workspace::Workspace,
]);

#[track_time]
pub fn run_default() -> ExitCode {
    let BinMeta {
        cwd,
        args,
        version,
    } = extract_bin_meta();

    if let Some(cwd) = cwd {
        cwd.sys_set_current_dir()
            .expect("Failed to set current directory");
    }

    let current_cwd = Path::current_dir()
        .expect("Failed to get current directory");

    let canonicalized_cwd = current_cwd
        .fn_canonicalize()
        .expect("Failed to canonicalize current directory");

    println!("Current directory: {}", current_cwd);
    println!("Canonicalized directory: {}", canonicalized_cwd);

    if current_cwd != canonicalized_cwd {
        println!("Current directory is not canonicalized, setting to: {}", canonicalized_cwd);

        canonicalized_cwd
            .sys_set_current_dir()
            .expect("Failed to set current directory");
    }

    let env
        = Environment::default()
            .with_program_name("Yarn Package Manager".to_string())
            .with_binary_name("yarn".to_string())
            .with_version(version)
            .with_argv(args);

    YarnCli::run(env)
}
