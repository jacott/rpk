use std::str::FromStr;

use super::*;

#[test]
fn test_get_config_filename() {
    let dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let input = TokenStream::from_str("").unwrap();
    assert_eq!(
        get_config_filename(input).unwrap(),
        dir.join("default-layout.rpk.conf")
    );

    let input = quote! {
        "../test.conf"
    };

    assert_eq!(
        get_config_filename(input).unwrap(),
        dir.join("../test.conf")
    );

    let input = quote! {
        "/test.conf"
    };

    assert_eq!(
        get_config_filename(input).unwrap(),
        PathBuf::from("/test.conf")
    );
}

#[test]
fn quote_conf_with_invalid_config() {
    const LAYOUT: &str = "test/invalid-layout.rpk.conf";
    let cargo = &PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());

    let filename = cargo.join(LAYOUT);
    let res = quote_conf(&filename).err().unwrap().to_string();

    assert!(res.starts_with("error: Invalid global 'foo'"), "{}", res);
    assert!(
        res.contains("rpk-macros/test/invalid-layout.rpk.conf:3:1\n"),
        "{}",
        res
    );
    assert!(res.trim().ends_with("3 | foo = 123"), "{}", res);
}

#[test]
fn quote_conf_with_valid_config() {
    const LAYOUT: &str = "test/default-layout.rpk.conf";
    let cargo = &PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());

    let filename = cargo.join(LAYOUT);
    let res = quote_conf(&filename).unwrap();

    let ast: syn::File = syn::parse2(res.clone()).unwrap();

    struct Visitor(Vec<String>);
    impl<'ast> Visit<'ast> for Visitor {
        fn visit_item_const(&mut self, i: &'ast syn::ItemConst) {
            self.0.push(i.ident.to_string());
        }
        fn visit_item_static(&mut self, i: &'ast syn::ItemStatic) {
            self.0.push(i.ident.to_string());
        }
    }

    let mut vis = Visitor(vec![]);
    vis.visit_file(&ast);
    assert_eq!(vis.0.len(), 14);
    assert_eq!(vis.0[0], "LAYOUT_MAPPING");
    assert_eq!(vis.0[1], "INPUT_N");
    assert_eq!(vis.0[10], "CONFIG_BUILDER");
    assert_eq!(vis.0[12], "REPORT_BUFFER_SIZE");
}

#[test]
fn test_syn_array_len() {
    let pins = quote! {[PIN_1, PIN_2,P3,]};
    assert_eq!(syn_array_len(&pins).unwrap(), 3);

    let pins = quote! {[
    PIN_1, PIN_2,
    P3, P4]};
    assert_eq!(syn_array_len(&pins).unwrap(), 4);
}

#[test]
fn test_syn_bool() {
    let pins = quote! {
        true // comment
    };
    assert!(syn_bool(&pins).unwrap());

    let pins = quote! {  false };
    assert!(!syn_bool(&pins).unwrap());
}
