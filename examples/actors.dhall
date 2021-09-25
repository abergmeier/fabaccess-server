{ Shelly_1234 = { module = "Shelly", params = {=} }
, Bash =
  { module = "Process"
  , params = { cmd = "./examples/actor.sh", args = "your ad could be here" }
  }
, Bash2 =
  { module = "Process"
  , params = { cmd = "./examples/actor.sh", args = "this is a different one" }
  }
, FailBash = { module = "Process", params.cmd = "./examples/fail-actor.sh" }
}
