extern crate proc_macro;

#[allow(unused)]
use rpk_config::fixme;

mod build;

#[proc_macro]
pub fn configure_keyboard(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    build::configure_keyboard(proc_macro2::TokenStream::from(input)).into()
}
