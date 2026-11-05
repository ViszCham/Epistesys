#![forbid(unsafe_code)]

fn main() {
    let owner = lc631_runtime::resource::ResourceLimits::checked([
        512 * 1024 * 1024,
        1024 * 1024 * 1024,
        128 * 1024 * 1024,
        64 * 1024 * 1024,
        2,
        4 * 1024 * 1024,
    ])
    .and_then(lc631_runtime::resource::ResourceBroker::new);
    match owner {
        Ok(owner) => lc631_cli::main_entry_with_resources(&owner),
        Err(error) => {
            eprintln!("resource_owner_initialization_failed:{error:?}");
            std::process::exit(2);
        }
    }
}
