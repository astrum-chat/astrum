fn main() {
    let src_dir = std::path::Path::new("src");
    let mut c = cc::Build::new();
    c.std("c11").include(src_dir);
    let parser = src_dir.join("parser.c");
    c.file(&parser);
    println!("cargo:rerun-if-changed={}", parser.to_str().unwrap());
    c.compile("tree-sitter-astrum-md");
}
