pub use lmdb::{
    Environment,

    DatabaseFlags,
    WriteFlags,
    EnvironmentFlags,

    Transaction,
    RoTransaction,
    RwTransaction,
};

mod raw;
use raw::RawDB;

mod typed;
// re-exports
pub use typed::{
    DB,
    Cursor,

    Adapter,
    OutputBuffer,
    OutputWriter,
};

mod hash;
pub use hash::{
    HashDB,
    Entry,
};

mod fix;
pub use fix::LMDBorrow;


#[cfg(test)]
mod tests {
    use super::*;
    use std::result::Result;
    use std::ops::Deref;

    use lmdb::{
        EnvironmentFlags as EF,
        DatabaseFlags as DF,
        WriteFlags as WF,
    };

    pub struct TempEnv {
        dir: tempfile::TempDir,
        env: Arc<Environment>,
    }

    impl Deref for TempEnv {
        type Target = Arc<Environment>;
        fn deref(&self) -> &Self::Target {
            &self.env
        }
    }

    pub fn open_test_env() -> TempEnv {
        let dir = tempfile::tempdir().expect("Failed to create tempdir for testdb");
        let env = Environment::new()
            .set_flags(EF::NO_SYNC | EF::WRITE_MAP)
            .open(dir.path()).expect("Failed to open lmdb");
        let env = Arc::new(env);

        TempEnv { dir, env }
    }

    struct TestAdapter;

    #[derive(Debug)]
    enum TestErr {
        Utf8(std::str::Utf8Error),
        Binc(Box<bincode::ErrorKind>),
        LMDB(lmdb::Error),
    }

    impl From<lmdb::Error> for TestErr {
        fn from(e: lmdb::Error) -> TestErr {
            TestErr::LMDB(e)
        }
    }

    impl From<std::str::Utf8Error> for TestErr {
        fn from(e: std::str::Utf8Error) -> TestErr {
            TestErr::Utf8(e)
        }
    }

    impl From<bincode::Error> for TestErr {
        fn from(e: bincode::Error) -> TestErr {
            TestErr::Binc(e)
        }
    }

    impl DatabaseAdapter for TestAdapter {
        type Key = str;
        type Err = TestErr;

        fn serialize_key(key: &Self::Key) -> &[u8] {
            key.as_bytes()
        }
        fn deserialize_key<'de>(input: &'de [u8]) -> Result<&'de Self::Key, Self::Err> {
            std::str::from_utf8(input).map_err(|e| e.into())
        }
    }

    type TestDB<'txn> = Objectstore<'txn, TestAdapter, &'txn str>;

    #[test]
    fn simple_get() {
        let e = open_test_env();
        let ldb = e.create_db(None, DF::empty()).expect("Failed to create lmdb db");

        let db = DB::new(e.env.clone(), ldb);

        let testdb = TestDB::new(db.clone());

        let mut val = "value";
        let mut txn = db.begin_rw_txn().expect("Failed to being rw txn");
        testdb.put(&mut txn, "key", &val, WF::empty()).expect("Failed to insert");
        testdb.put(&mut txn, "key2", &val, WF::empty()).expect("Failed to insert");
        testdb.put(&mut txn, "key3", &val, WF::empty()).expect("Failed to insert");
        testdb.put(&mut txn, "key4", &val, WF::empty()).expect("Failed to insert");
        testdb.put(&mut txn, "key5", &val, WF::empty()).expect("Failed to insert");
        txn.commit().expect("commit failed");

        {
            let txn;
            txn = db.begin_ro_txn().unwrap();

            let val = testdb.get(&txn, "key").expect("Failed to retrieve");
            assert_eq!(Some("value"), val);
        }

        {
            let val2 = "longer_value";
            let mut txn = db.begin_rw_txn().unwrap();
            testdb.put(&mut txn, "key", &val2, WF::empty()).expect("Failed to update");
            txn.commit().unwrap();
        }

        {
            let txn = db.begin_ro_txn().unwrap();
            let found = testdb.get_in_place(&txn, "key", &mut val).expect("Failed to retrieve update");
            assert!(found);
            assert_eq!("longer_value", val);
        }

        {
            let txn = db.begin_ro_txn().unwrap();
            let mut it = testdb.iter(&txn).unwrap();
            assert_eq!("longer_value", it.next().unwrap().unwrap());
            let mut i = 0;
            while let Some(e) = it.next() {
                assert_eq!("value", e.unwrap());
                i += 1;
            }
            assert_eq!(i, 4)
        }
    }
}
