extern crate proc_macro;

use rpk_config::compiler::compile;

mod build;

#[proc_macro]
pub fn configure_keyboard(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    build::configure_keyboard(proc_macro2::TokenStream::from(input)).into()
}

#[proc_macro]
pub fn layout_config(conf: proc_macro::TokenStream) -> proc_macro::TokenStream {
    fn error(message: &str) -> proc_macro::TokenStream {
        format!(
            r####"{{compile_error!(r###"{}"###); const M: [u16; 0] = [];&M}}"####,
            message
        )
        .parse()
        .unwrap()
    }

    if let Some(proc_macro::TokenTree::Literal(item)) = conf.into_iter().next() {
        let conf = item.to_string();
        let mut conf = conf.as_str();
        if conf.starts_with('r') {
            if let Some(i) = conf.find('"') {
                conf = &conf[i..conf.len() - i + 1];
            }
        }

        if conf.starts_with('"') {
            conf = &conf[1..conf.len() - 1];
        }
        match compile(conf) {
            Ok(source) => {
                let bin = source.serialize();
                format!("{{ const M: [u16; {}] = {:?}; &M }}", bin.len(), bin)
                    .parse()
                    .unwrap()
            }
            Err(err) => error(err.to_string().as_str()),
        }
    } else {
        error("Expected a string argument")
    }
}
