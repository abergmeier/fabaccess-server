use crate::resources::Resource;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
struct Inner {
    id: HashMap<String, Resource>,
}

impl Inner {
    pub fn new(resources: impl IntoIterator<Item = Resource>) -> Self {
        let mut id = HashMap::new();

        for resource in resources {
            let old = id.insert(resource.inner.id.clone(), resource);
            assert!(old.is_none());
        }

        Self { id }
    }
}

#[derive(Clone, Debug)]
pub struct ResourcesHandle {
    inner: Arc<Inner>,
}

impl ResourcesHandle {
    pub fn new(resources: impl IntoIterator<Item = Resource>) -> Self {
        Self {
            inner: Arc::new(Inner::new(resources)),
        }
    }

    pub fn list_all(&self) -> impl IntoIterator<Item = &Resource> {
        self.inner.id.values()
    }

    pub fn get_by_id(&self, id: &str) -> Option<&Resource> {
        self.inner.id.get(id)
    }

    pub fn get_by_urn(&self, urn: &str) -> Option<&Resource> {
        if let Some(id) = {
            let mut parts = urn.split_terminator(':');
            let part_urn = parts.next().map(|u| u == "urn").unwrap_or(false);
            let part_fabaccess = parts.next().map(|f| f == "fabaccess").unwrap_or(false);
            let part_resource = parts.next().map(|r| r == "resource").unwrap_or(false);
            if !(part_urn && part_fabaccess && part_resource) {
                return None;
            }
            parts.next().map(|s| s.to_string())
        } {
            self.get_by_id(&id)
        } else {
            None
        }
    }
}
