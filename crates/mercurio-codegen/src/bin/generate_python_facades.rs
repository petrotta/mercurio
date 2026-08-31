use std::path::PathBuf;

use mercurio_codegen::generate_python_facades;
use mercurio_foundation::KirDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = PathBuf::from(args.next().ok_or(
        "usage: generate_python_facades <metamodel.kir.json> <output-root> <module-name>",
    )?);
    let output_root = PathBuf::from(args.next().ok_or(
        "usage: generate_python_facades <metamodel.kir.json> <output-root> <module-name>",
    )?);
    let module_name = args
        .next()
        .ok_or("usage: generate_python_facades <metamodel.kir.json> <output-root> <module-name>")?;
    if args.next().is_some() {
        return Err("unexpected extra arguments".into());
    }

    let document: KirDocument = serde_json::from_slice(&std::fs::read(input)?)?;
    let generated = generate_python_facades(&document, &module_name);
    for (relative_path, contents) in generated.files {
        let path = output_root.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)?;
    }
    Ok(())
}
