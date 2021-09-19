-- { actor_connections = [] : List { _1 : Text, _2 : Text }
{ actor_connections = [{ _1 = "Testmachine", _2 = "Actor" }]
, actors = 
  { Actor = { module = "Shelly", params = {=} }
  }
  , init_connections = [] : List { _1 : Text, _2 : Text }
--, init_connections = [{ _1 = "Initiator", _2 = "Testmachine" }]
, initiators = 
  { Initiator = { module = "Dummy", params = {=} } 
  }
, listens = 
  [ { address = "::", port = Some 59661 }
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
, mqtt_url = "tcp://mqtt:1883" 
, db_path = "/tmp/bffh"
}
