fn main() {
    if let Some(env) = std::env::var_os("TARGET") {
        println!("cargo:rustc-env=TARGET={}", env.to_string_lossy());
    }
}