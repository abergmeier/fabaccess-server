# API-Testsetup

wirklich nur um das API zu testen. ATM implementiert: machines::* & machine::read, authenticate

1. Ein mosquitto o.ä MQTT Server starten
1. Datenbanken füllen: `cargo run -- -c examples/bffh.dhall --load=examples`
1. Daemon starten: `cargo run -- -c examples/bffh.dhall`
1. ???
1. PROFIT!

A dockerized version of this example can be found in the docker subdirectory