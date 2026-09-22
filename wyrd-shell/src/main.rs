use anyhow::Result;
use clap::Parser;

use wyrd_shell::{cli, runtime};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(unix)]
fn daemonize_process() -> Result<()> {
    // SAFETY: standard double-fork/daemonization sequence before threads are spawned.
    // Calls POSIX libc primitives `fork()`, `setsid()`, `open()`, `dup2()`, and `close()`.
    unsafe {
        let pid = libc::fork();
        if pid < 0 {
            anyhow::bail!("fork() failed");
        }
        if pid > 0 {
            std::process::exit(0);
        }
        if libc::setsid() < 0 {
            anyhow::bail!("setsid() failed");
        }
        let devnull = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR);
        if devnull >= 0 {
            libc::dup2(devnull, libc::STDIN_FILENO);
            libc::dup2(devnull, libc::STDOUT_FILENO);
            libc::dup2(devnull, libc::STDERR_FILENO);
            if devnull > libc::STDERR_FILENO {
                libc::close(devnull);
            }
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn daemonize_process() -> Result<()> {
    anyhow::bail!("daemonize is only supported on Unix targets");
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    // SAFETY: Tuning mimalloc global allocator options and glibc allocator parameters (mallopt)
    // at early process startup before allocating objects across multiple threads.
    unsafe {
        libmimalloc_sys::mi_option_set(15, 0);
        libmimalloc_sys::mi_option_set(24, 1);
        libmimalloc_sys::mi_option_set(libmimalloc_sys::mi_option_large_os_pages, 0);
        libmimalloc_sys::mi_option_set(libmimalloc_sys::mi_option_reserve_huge_os_pages, 0);
        libmimalloc_sys::mi_option_set(37, 0);
        libmimalloc_sys::mi_option_set(4, 0);
        #[cfg(target_os = "linux")]
        libc::mallopt(-1, 2);
    }

    let cli = cli::Cli::parse();

    if let Some(shell) = &cli.completions {
        match shell.as_str() {
            "bash" => print!("{}", include_str!("../../completions/wyrd-shell.bash")),
            "zsh" => print!("{}", include_str!("../../completions/_wyrd-shell")),
            "fish" => print!("{}", include_str!("../../completions/wyrd-shell.fish")),
            _ => eprintln!("Unsupported shell: {}", shell),
        }
        std::process::exit(0);
    }

    if cli.doctor || cli.action.as_deref() == Some("doctor") {
        let items = wyrd_shell::doctor::collect_diagnostics();
        let ok = wyrd_shell::doctor::print_diagnostics(&items);
        std::process::exit(if ok { 0 } else { 1 });
    }

    let mut action_to_send = cli.action.clone();
    if let Some(t) = &cli.theme {
        action_to_send = Some(format!("theme:set {}", t));
    } else if let Some(l) = &cli.layout {
        action_to_send = Some(format!("layout:set {}", l));
    }

    if let Some(action) = action_to_send {
        let sock_path = runtime::socket::shell_socket_path();
        use tokio::io::AsyncWriteExt;
        match tokio::net::UnixStream::connect(&sock_path).await {
            Ok(mut stream) => {
                if let Err(e) = stream.write_all(format!("{}\n", action).as_bytes()).await {
                    eprintln!("Error sending action to wyrd-shell: {}", e);
                    std::process::exit(1);
                }
                std::process::exit(0);
            }
            Err(e) => {
                if action.starts_with("theme:set ") {
                    let target = action.trim_start_matches("theme:set ").trim();
                    let _ = runtime::socket::update_settings_file(|t| {
                        t.insert("theme".to_string(), toml::Value::String(target.to_string()));
                    });
                    println!("Theme saved as '{}' in settings.toml", target);
                    std::process::exit(0);
                } else if action.starts_with("layout:set ") {
                    let target = action.trim_start_matches("layout:set ").trim();
                    let _ = runtime::socket::update_settings_file(|t| {
                        t.insert(
                            "layout".to_string(),
                            toml::Value::String(target.to_string()),
                        );
                    });
                    println!("Layout saved as '{}' in settings.toml", target);
                    std::process::exit(0);
                }
                eprintln!(
                    "Failed to connect to wyrd-shell daemon at {}: {}",
                    sock_path.display(),
                    e
                );
                eprintln!("Is wyrd-shell running?");
                std::process::exit(1);
            }
        }
    }

    let env = env_logger::Env::default().default_filter_or(&cli.log_level);
    env_logger::Builder::from_env(env)
        .format_timestamp_secs()
        .init();

    log::info!("Wyrd Shell starting...");

    if cli.daemonize {
        daemonize_process()?;
    }

    runtime::run(cli).await
}
