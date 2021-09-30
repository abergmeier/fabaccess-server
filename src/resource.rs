use core::sync::atomic;

/// A something BFFH holds internal state of
pub struct Resource {
    // claims
    strong: atomic::AtomicUsize,
    weak: atomic::AtomicUsize,
    max_strong: usize,
}

/// A claim is taken in lieu of an user on a resource. 
///
/// They come in two flavours: Weak, of which an infinite amount can exist, and Strong which may be
/// limited in number. Strong claims represent the right of the user to use this resource
/// "writable". A weak claim indicates co-usage of a resource and are mainly useful for notice and
/// information of the respective other ones. E.g. a space would be strongly claimed by keyholders
/// when they check in and released when they check out and weakly claimed by everybody else. In
/// that case the last strong claim could also fail to be released if there are outstanding weak
/// claims. Alternatively, releasing the last strong claim also releases all weak claims and sets
/// the resource to "Free" again.
///
/// Most importantly, claims can be released by *both* the claim holder and the resource.
pub struct Claim {
    id: u128,
}
