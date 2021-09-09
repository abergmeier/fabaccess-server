use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Space {
    pub id: Uuid,
    pub name: String,
    pub info: String,
}

