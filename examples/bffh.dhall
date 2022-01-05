-- { actor_connections = [] : List { _1 : Text, _2 : Text }
{ actor_connections = 
  -- Link up machines to actors
  [ { machine = "Testmachine", actor = "Shelly1234" }
  , { machine = "Another", actor = "Bash" }
  -- One machine can have as many actors as it wants
  , { machine = "Yetmore", actor = "Bash2" }
  , { machine = "Yetmore", actor = "FailBash"}
  ]
, actors = 
  { Shelly1234 = { module = "Shelly", params =
    { topic = "Topic1234" }}
  , Bash = { module = "Process", params =
    { cmd = "./examples/actor.sh"
    , args = "your ad could be here"
    }}
  , Bash2 = { module = "Process", params =
    { cmd = "./examples/actor.sh"
    , args = "this is a different one"
    }}
  , FailBash = { module = "Process", params =
    { cmd = "./examples/fail-actor.sh" 
    }}
  }
  , init_connections = [] : List { machine : Text, initiator : Text }
  --, init_connections = [{ machine = "Testmachine", initiator = "Initiator" }]
  , initiators = {=}
  --{ Initiator = { module = "Dummy", params = { uid = "Testuser" } } }
, listens = 
  [ { address = "127.0.0.1", port = Some 59661 }
  , { address = "::1", port = Some 59661 }
  , { address = "192.168.0.114", port = Some 59661 }
  ]
, machines = 
  { Testmachine = 
    { description = "A test machine"
    , wiki = "test"
    , disclose = "lab.test.read"
    , manage = "lab.test.admin"
    , name = "MachineA"
    , read = "lab.test.read"
    , write = "lab.test.write" 
    },
    Another = 
    { wiki = "test_another"
    , category = "test"
    , disclose = "lab.test.read"
    , manage = "lab.test.admin"
    , name = "Another"
    , read = "lab.test.read"
    , write = "lab.test.write" 
    },
    Yetmore = 
    { description = "Yet more test machines"
    , disclose = "lab.test.read"
    , manage = "lab.test.admin"
    , name = "Yetmore"
    , read = "lab.test.read"
    , write = "lab.test.write" 
    }
  }
, mqtt_url = "tcp://localhost:1883" 
, db_path = "/tmp/bffh"
, roles =
  { testrole = 
    { permissions = [ "lab.test.*" ] }
  , somerole = 
    { parents = ["testparent"]
    , permissions = [ "lab.some.admin" ]
    }
  , testparent = 
    { permissions = 
      [ "lab.some.write"
      , "lab.some.read"
      , "lab.some.disclose"
      ]
    }
  }
, certfile = "examples/self-signed-cert.pem"
, keyfile = "examples/self-signed-key.pem"
}
