# API-Testsetup

wirklich nur um das API zu testen. ATM implementiert: machine::read

1. `cargo run -- --print-default > /tmp/bffh.toml` um eine default config zu generieren
1. in /tmp/bffh.toml den parameter `machines` auf ./examples/machines.toml umbiegen
    * Bei mir z.b. `~/Development/FabInfra/Diflouroborane/examples/machines.toml`
1. Ein mosquitto o.ä MQTT Server starten
    * Bringt aber leider gerade nicht viel ^^'
1. `cargo run -- -c /tmp/bffh.toml`
1. ???
1. PROFIT!
