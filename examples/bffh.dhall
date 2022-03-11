{ actor_connections =
    -- Link up machines to actors
    [ { _1 = "Testmachine", _2 = "Shelly_1234" }
    , { _1 = "Another", _2 = "Bash" }
    , { _1 = "Yetmore", _2 = "Bash2" }
    , { _1 = "Yetmore", _2 = "FailBash" }
    ]
, actors = ./actors.dhall
, init_connections = [] : List { _1 : Text, _2 : Text }
, initiators = ./initiators.dhall
, listens =
  [ "127.0.0.1"
  , "::1"
  , "[::1]:1235"
  , "localhost:1234"
  , "localhost"
  , "notahost:541"
  , "notahostandnoport"
  ]
, machines = ./machines.dhall
, db_path = "/tmp/bffh/"
, roles = ./roles.dhall
, mqtt_url = "tcp://localhost:1883"
, certfile = "./bffh.crt"
, keyfile = "./bffh.key"
}
