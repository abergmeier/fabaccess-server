use smol::Task;

pub struct Initiator {
    inner: Task<()>,
}

pub fn load(config: &crate::config::Settings) -> Result<Vec<Initiator>> {
    unimplemented!()
}
