use std::sync::Arc;
use crate::resources::Resource;

struct Inner {

}

impl Inner {
    pub fn new() -> Self {
        Self { }
    }
}

#[derive(Clone)]
pub struct ResourcesHandle {
    inner: Arc<Inner>,
}

impl ResourcesHandle {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner::new()),
        }
    }

    pub fn list_all(&self) -> impl IntoIterator<Item=&Resource> {
        unimplemented!();
        &[]
    }

    pub fn get_by_id(&self, id: &str) -> Option<&Resource> {
        unimplemented!()
    }

    pub fn get_by_urn(&self, urn: &str) -> Option<&Resource> {
        unimplemented!()
    }
}
