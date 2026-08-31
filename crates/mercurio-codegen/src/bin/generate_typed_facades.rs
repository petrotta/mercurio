use std::env;
use std::fs;
use std::path::PathBuf;

use mercurio_codegen::generate_typed_facades;
use mercurio_foundation::KirDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("missing input KIR path")?;
    let output = env::args()
        .nth(2)
        .map(PathBuf::from)
        .ok_or("missing output Rust path")?;
    let document: KirDocument = serde_json::from_slice(&fs::read(input)?)?;
    let generated = generate_typed_facades(&document)?;
    fs::write(output, generated)?;
    Ok(())
}
