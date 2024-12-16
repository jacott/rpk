#[allow(unused)]
#[cfg(all(not(test), not(feature = "defmt")))]
mod no_defmt {
    #[macro_export]
    macro_rules! fixme {
        ($a:expr) => {{
            let _ = $a;
        }};
    }

    #[macro_export]
    macro_rules! info {
    ($($arg:expr),*) => {{let _ = ($($arg),*);}};
}

    #[macro_export]
    macro_rules! debug {
    ($($arg:expr),*) => {{let _ = ($($arg),*);}};
}

    #[macro_export]
    macro_rules! warn {
    ($($arg:expr),*) => {{let _ = ($($arg),*);}};
}

    #[macro_export]
    macro_rules! error {
    ($($arg:expr),*) => {{let _ = ($($arg),*);}};
}
}

#[cfg(all(not(test), feature = "defmt"))]
mod defmt {
    #[macro_export]
    macro_rules! fixme {
        ($a:expr) => {
            defmt::debug!(
                // split so that not found when looking for the fixme ! in an editor
                "FIXME: at {}:{}:{}\n{:?}",
                file!(),
                line!(),
                column!(),
                $a,
            )
        };
    }

    #[macro_export]
    macro_rules! debug {
        ($($arg:expr),*) => {
            defmt::debug!($($arg,)*)
        };
    }

    #[macro_export]
    macro_rules! info {
        ($($arg:expr),*) => {
            defmt::info!($($arg,)*)
        };
    }

    #[macro_export]
    macro_rules! warn {
        ($($arg:expr),*) => {
            defmt::warn!($($arg,)*)
        };
    }

    #[macro_export]
    macro_rules! error {
        ($($arg:expr),*) => {
            defmt::info!($($arg,)*)
        };
    }
}

#[cfg(test)]
mod test {
    #[macro_export]
    macro_rules! kc {
        ($a:expr) => {
            match rpk_config::keycodes::key_code($a) {
                Some(kc) => kc,
                None => panic!("Unknown key mnemonic: {:?}", $a),
            }
        };
    }

    #[macro_export]
    macro_rules! fixme {
        ($a:expr) => {{
            extern crate std;
            std::eprintln!(
                // split so that not found when looking for the word in an editor
                "FIXME\
             ! at ./{}:{}:{}\n{:?}",
                file!(),
                line!(),
                column!(),
                $a,
            )
        }};
    }

    #[macro_export]
    macro_rules! debug {
    ($($arg:expr),*) => {{
        extern crate std;
        std::eprintln!("DEBUG: {}",  format_args!($($arg,)*))
    }};
}

    #[macro_export]
    macro_rules! info {
    ($($arg:expr),*) => {{
        extern crate std;
        std::eprintln!("INFO: {}",  std::format!($($arg,)*))
    }};
}

    #[macro_export]
    macro_rules! warn {
    ($($arg:expr),*) => {{
        extern crate std;
        std::eprintln!("WARN: {}",  std::format!($($arg,)*))
    }};
}

    #[macro_export]
    macro_rules! error {
    ($($arg:expr),*) => {{
        extern crate std;
        std::eprintln!("\nERROR: at ./{}:{}:{}:\n{}", file!(), line!(), column!(),
            std::format!($($arg,)*))
    }};
}
}
