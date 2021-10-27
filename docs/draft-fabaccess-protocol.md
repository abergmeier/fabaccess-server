# Stream initiation

In a session there are two parties: The initiating entity and the receiving
entity.  This terminology does not refer to information flow but rather to the
side opening a connection respectively the one listening for connection
attempts.
In the currently envisioned use-case the initiating entity is a) a client
(i.e. interactive or batch/automated program) trying to interact in some way or
other with a server b) a server trying to exchange / request information
with/from another server (i.e. federating).  The receiving entity however is
already a server.

Additionally the amount and type of clients is likely to be more diverse and
less up to date than the servers.  
Conclusions I draw from this:
  - Clients are more likely to implement an outdated version of the communication
      protocol.
  - The place for backwards-compatability should be the servers.
  - Thus the client (initiating entity) should send the expected API version
      first, the server then using that as a basis to decide with which API
      version to answer.

# Stream negotiation

Since the receiving entity for a connection is responsible for the machines it
controls it imposes conditions for connecting either as client or as federating
server.  At least every initiating entity is required to authenticate itself to
the receiving entity before attempting further actions or requesting
information.  But a receiving entity can require other features, such as
transport layer encryption. 
To this end a receiving entity informs the initiating entity about features that
it requires from the initiating entity before taking any further action and
features that are voluntary to negotiate but may improve qualities of the stream
(such as message compression)

A varying set of conditions implies negotiation needs to take place.  Since
features potentially require a strict order (e.g. Encryption before
Authentication) negotiation has to be a multi-stage process. Further
restrictions are imposed because some features may only be offered after others
have been established (e.g. SASL authentication only becoming available after
encryption, EXTERNAL mechanism only being available to local sockets or
connections providing a certificate)
