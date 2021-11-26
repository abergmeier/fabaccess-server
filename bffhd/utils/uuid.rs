use uuid::Uuid;
use api::utils::uuid::{Builder, Reader};

pub fn uuid_to_api(uuid: Uuid, mut builder: Builder) {
    let [a,b,c,d,e,f,g,h,i,j,k,l,m,n,o,p]
        = uuid.as_u128().to_ne_bytes();
    let lower = u64::from_ne_bytes([a,b,c,d,e,f,g,h]);
    let upper = u64::from_ne_bytes([i,j,k,l,m,n,o,p]);
    builder.set_lower(lower);
    builder.set_upper(upper);
}

pub fn api_to_uuid(reader: Reader) -> Uuid {
    let lower: u64 = reader.reborrow().get_lower();
    let upper: u64 = reader.get_upper();
    let [a,b,c,d,e,f,g,h] = lower.to_ne_bytes();
    let [i,j,k,l,m,n,o,p] = upper.to_ne_bytes();
    let num = u128::from_ne_bytes([a,b,c,d,e,f,g,h,i,j,k,l,m,n,o,p]);
    Uuid::from_u128(num)
}