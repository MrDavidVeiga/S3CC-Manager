use std::{env, path::Path};

fn main() {
    let mut args = env::args().skip(1);
    let store = args.next().unwrap_or_default();
    let official = args.next().unwrap_or_default();
    if store.is_empty() || official.is_empty() {
        eprintln!("usage: reference-catalog-builder <store-root> <official-root>");
        std::process::exit(2);
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
