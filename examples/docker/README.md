# API-Testsetup, aber mit Docker

wirklich nur um das API zu testen. ATM implementiert: machine::read

* Use included config.toml, or
* generate a sample config.toml by running "docker run registry.gitlab.com/fabinfra/fabaccess/bffh:dev-latest --print-default > config.toml". You may have to delete the ipv6 listen section. 
    1. change mqtt-server hostname to `mqtt` in config.toml
    1. change machines path to `/etc/bffh/machines.toml`

* run `docker-compose up`
