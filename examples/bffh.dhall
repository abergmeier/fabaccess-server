-- { actor_connections = [] : List { _1 : Text, _2 : Text }
{ actor_connections = 
  -- Link up machines to actors
  [ { _1 = "Testmachine", _2 = "Actor" }
  , { _1 = "Another", _2 = "Bash" }
  -- One machine can have as many actors as it wants
  , { _1 = "Yetmore", _2 = "Bash2" }
  , { _1 = "Yetmore", _2 = "FailBash"}
  ]
, actors = 
  { Actor = { module = "Dummy", params = {=} }
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
  , init_connections = [] : List { _1 : Text, _2 : Text }
--, init_connections = [{ _1 = "Initiator", _2 = "Testmachine" }]
, initiators = --{=}
  { Initiator = { module = "Dummy", params = {=} } }
, listens = 
  [ { address = "127.0.0.1", port = Some 59661 }
  , { address = "::1", port = Some 59661 }
  , { address = "192.168.0.114", port = Some 59661 }
  ]
, machines = 
  { Testmachine = 
    { description = Some "A test machine"
    , disclose = "lab.test.read"
    , manage = "lab.test.admin"
    , name = "Testmachine"
    , read = "lab.test.read"
    , write = "lab.test.write" 
    },
    Another = 
    { description = Some "Another test machine"
    , disclose = "lab.test.read"
    , manage = "lab.test.admin"
    , name = "Another"
    , read = "lab.test.read"
    , write = "lab.test.write" 
    },
    Yetmore = 
    { description = Some "Yet more test machines"
    , disclose = "lab.test.read"
    , manage = "lab.test.admin"
    , name = "Yetmore"
    , read = "lab.test.read"
    , write = "lab.test.write" 
    }
  }
, mqtt_url = "" 
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
}
