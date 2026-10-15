use std::{env, path::Path};

fn main() {
    let mut args = env::args().skip(1);
    let store = args.next().unwrap_or_default();
    let official = args.next().unwrap_or_default();
    if store.is_empty() || official.is_empty() {
        eprintln!("usage: reference-catalog-builder <store-root> <official-root>");
        std::process::exit(2);
    }

    // The desktop app uses the platform data directories by default.  The
    // standalone builder can be pointed at a stable local app-data directory
    // so its persistent catalogs/cache are not redirected into a disposable
    // sandbox by the host running this utility.
    if std::env::var_os("S3CC_MANAGER_DATA_ROOT").is_none() {
        eprintln!("S3CC_MANAGER_DATA_ROOT is not set; using the platform app-data directory");
    }

    match s3cc_organizer_lib::reference_catalog::build_reference_catalogs_sync(
        Path::new(&store),
        Path::new(&official),
    ) {
        Ok(result) => println!(
            "{}",
            serde_json::to_string_pretty(&result).unwrap_or_default()
        ),
        Err(error) => {
            eprintln!("reference catalog build failed: {error:#}");
            std::process::exit(1);
        }
    }
}
