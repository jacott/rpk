use std::{collections::HashMap, str::FromStr};

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

    struct Visitor(HashMap<String, String>);
    impl<'ast> Visit<'ast> for Visitor {
        fn visit_item_const(&mut self, i: &'ast syn::ItemConst) {
            self.0.insert(
                i.ident.to_string(),
                i.expr.to_token_stream().to_string().replace(" ", ""),
            );
        }
        fn visit_item_static(&mut self, i: &'ast syn::ItemStatic) {
            self.0.insert(
                i.ident.to_string(),
                i.expr.to_token_stream().to_string().replace(" ", ""),
            );
        }
    }

    let mut vis = Visitor(HashMap::new());
    vis.visit_file(&ast);
    assert_eq!(vis.0.len(), 14);
    assert_eq!(vis.0.get("LAYOUT_MAPPING").unwrap(),
        "{constM:[u16;29]=[1,771,7,0,0,8,9,10,11,12,13,23,24,1,2,4,8,64,0,36,37,38,33,34,35,30,31,32,0];&M}");

    assert_eq!(vis.0.get("INPUT_N").unwrap(), "3usize");
    assert_eq!(vis.0.get("FS_SIZE").unwrap(), "FLASH_SIZE-FS_BASE");

    let cfg = vis.0.get("CONFIG_BUILDER").unwrap();

    assert!(cfg.contains("vendor_id:0x6e0f"));
    assert!(cfg.contains("serial_number:\"rpk:0001\""));
    assert!(cfg.contains("max_power:100"));
    assert_eq!(vis.0.get("REPORT_BUFFER_SIZE").unwrap(), "32");
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
