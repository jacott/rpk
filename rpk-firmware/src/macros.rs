#[allow(unused)]
#[cfg(all(not(test), not(feature = "defmt"), not(feature = "test-utils")))]
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
    /// Convience macro to use whilst debugging code. It will call the [defmt::debug] macro.
    ///
    /// This macro works with either defmt or nothing. When testing on the host Operating system
    /// `eprintln!` will be called. In order to work with both `defmt` and `eprintln` the argument must
    /// derive `Debug`.
    ///
    /// # Example
    ///
    /// ```
    /// # #[macro_use] extern crate rpk_firmware;
    /// # fn main() {
    /// let i = 123;
    /// let j = "abc";
    /// fixme!(("test", i, j));
    /// # }
    /// ```
    #[macro_export]
    macro_rules! fixme {
        ($a:expr) => {
            defmt::debug!("FIXME: at {}:{}:{}\n{:?}", file!(), line!(), column!(), $a,)
        };
    }

    /// Log debug messages. It will call the [defmt::debug] macro.
    ///
    /// This macro works with either defmt or nothing. When testing on the host Operating system `eprintln!`
    /// will be called. In order to work with both `defmt` and `eprintln` only the debug syntax can be used;
    /// not the [defmt::Formatter] syntax.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[macro_use] extern crate rpk_firmware;
    /// # fn main() {
    /// let i = 123;
    /// let j = "abc";
    /// debug!("testing {}, {:?}", i, j);
    /// # }
    /// ```
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

#[cfg(feature = "test-utils")]
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
        if cfg!(test) {
            panic!("{}", std::format!($($arg,)*));
        } else {
            std::eprintln!("\nERROR: at ./{}:{}:{}:\n{}", file!(), line!(), column!(), std::format!($($arg,)*));
        }
    }};
}
}
