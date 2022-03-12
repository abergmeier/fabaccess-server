use std::sync::Arc;

struct Inner {

}
impl Inner {
    pub fn new() -> Self {
        Self { }
    }
}

#[derive(Clone)]
pub struct SessionManager {
    inner: Arc<Inner>,
}
impl SessionManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner::new()),
        }
    }
    pub fn open(&self, uid: impl AsRef<str>) -> Option<SessionHandle> {
        unimplemented!()
    }
}

#[derive(Clone, Debug)]
pub struct SessionHandle {
}

impl SessionHandle {
}