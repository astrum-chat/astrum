use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_astrum_md() -> *const ();
}

pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_astrum_md) };

pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");
