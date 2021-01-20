{ actor_connections = [{ _1 = "Testmachine", _2 = "Actor" }]
, actors = 
  { Actor = { module = "Shelly", params = {=} }
  }
, init_connections = [{ _1 = "Initiator", _2 = "Testmachine" }]
, initiators = 
  { Initiator = { module = "Dummy", params = {=} } 
  }
, listens = 
  [ { address = "127.0.0.1", port = Some 59661 }
  , { address = "::1", port = Some 59661 }
  ]
, machines = 
  { Testmachine = 
    { description = Some "A test machine"
    , disclose = "lab.test.read"
    , manage = "lab.test.admin"
    , name = "Testmachine"
    , read = "lab.test.read"
    , write = "lab.test.write" 
    } }
, mqtt_url = "tcp://localhost:1883" 
}
