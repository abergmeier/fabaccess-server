#[forbid(private_in_public)]
pub use sdk_proc::module;

pub use futures_util::future::BoxFuture;
pub mod initiators;

pub const VERSION_STRING: &'static str = env!("CARGO_PKG_VERSION");
pub const VERSION_STRING_PARTS: (&'static str, &'static str, &'static str, &'static str) = (
    env!("CARGO_PKG_VERSION_MAJOR"),
    env!("CARGO_PKG_VERSION_MINOR"),
    env!("CARGO_PKG_VERSION_PATCH"),
    env!("CARGO_PKG_VERSION_PRE"),
);

pub static VERSION_MAJOR: u32 = 0;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
