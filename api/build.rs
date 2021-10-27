use walkdir::{WalkDir, DirEntry};

fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

fn main() {
    // Tell cargo to only run this script if the schema files or this script have changed
    println!("cargo:rerun-if-changed=schema");

    let mut compile_command = ::capnpc::CompilerCommand::new();

    // Set parent module of all generated schema files.
    // i.e. a file "user.capnp" will result in module "schema::user"
    compile_command.default_parent_module(vec!["schema".into()]);

    for entry in WalkDir::new("schema")
        .max_depth(2)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
        .filter_map(Result::ok) // Filter all entries that access failed on
        .filter(|e| !e.file_type().is_dir()) // Filter directories
        // Filter non-schema files
        .filter(|e| e.file_name()
                     .to_str()
                     .map(|s| s.ends_with(".capnp"))
                     .unwrap_or(false)
        )
    {
        println!("Collecting schema file {}", entry.path().display());
        compile_command.file(entry.path());
    }

    println!("Compiling schemas...");
    compile_command.run().expect("Failed to generate API code");
}
