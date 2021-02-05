# Integration tests with Docker

## How it works
* spawns 2 instances of our bffh container and required mqqt broker 
* spawns an additional debian to run a shell
* the containers can reach each other by their hostname

## How to start

* run `docker-compose up --exit-code-from test-manager` in this directory
* this will kill all containers when 