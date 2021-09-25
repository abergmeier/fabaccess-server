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
  [ { address = "127.0.0.1", port = Some 59661 }
  , { address = "::1", port = Some 59661 }
  , { address = "192.168.0.114", port = Some 59661 }
  ]
, machines = ./machines.dhall
, db_path = "/tmp/bffh"
, roles = ./roles.dhall
, mqtt_url = "tcp://localhost:1883"
}
