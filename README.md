# FabAccess Difluoroborane

Difluoroborane (shorter: BFFH, the chemical formula for Difluoroborane) is the server part of
FabAccess.
It provides a server-side implementation of the [FabAccess API](https://gitlab.com/fabinfra/fabaccess/fabaccess-api).

## What is this?

FabAccess is a prototype-grade software suite for managing access, mostly aimed
at Makerspaces, FabLabs, and other open workshops.  It is designed to allow secure access control to
machines and other equipment that is dangerous or expensive to use. It tries to also be cheap enough
to be used for all other things one would like to give exclusive access to even when they are not
dangerous or expensive to use (think 3D printers, smart lightbulbs, meeting rooms).

FabAccess uses a Client/Server architecture with a [Cap'n Proto](https://capnproto.org/) API. You
can find the API schema files over [in their own repository](https://gitlab.com/fabinfra/fabaccess/fabaccess-api).
The reference client is [Borepin](https://gitlab.com/fabinfra/fabaccess/borepin), written in C#/Xamarin to be able to
be ported to as many platforms as possible.


## Installation

See [INSTALL.md](INSTALL.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Thanks!
